//! Compiler evidence: one JSON line per compiler row of the support matrix facts.

use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

const PROBE_FIELD: &str = "field cc.compiler=";

/// Where one evidence run reads its inputs and what it stamps on every line.
pub struct Run<'a> {
    pub platform: &'a str,
    pub bin: &'a Path,
    pub testlogs: &'a Path,
    pub lockfile: &'a Path,
    pub bazel: &'a str,
    pub command: &'a str,
    pub sha: &'a str,
    pub job: &'a str,
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn labels<'a>(row: &'a Value, keys: &[&str]) -> Vec<&'a str> {
    keys.iter()
        .filter_map(|key| row[*key].as_array())
        .flatten()
        .map(text)
        .filter(|label| !label.is_empty())
        .collect()
}

/// The output path of a label under an output root, with `suffix` appended to the file name.
fn output(root: &Path, label: &str, suffix: &str) -> PathBuf {
    let path = label
        .strip_prefix("//")
        .unwrap_or(label)
        .replacen(':', "/", 1);
    root.join(format!("{path}{suffix}"))
}

fn outcome_of(testlogs: &Path, label: &str) -> &'static str {
    let xml = output(testlogs, label, "/test.xml");
    let Ok(bytes) = std::fs::read(&xml) else {
        return "skipped";
    };
    let log = String::from_utf8_lossy(&bytes);
    if log.contains("<failure") || log.contains("<error") {
        "failed"
    } else {
        "passed"
    }
}

fn probe_lines(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(found) if found.contains(PROBE_FIELD) => {
            out.extend(found.lines().map(str::to_owned));
        }
        Value::Array(items) => items.iter().for_each(|item| probe_lines(item, out)),
        Value::Object(fields) => fields.values().for_each(|item| probe_lines(item, out)),
        _ => {}
    }
}

fn probe_field(lines: &[String], name: &str) -> String {
    let prefix = format!("field cc.{name}=");
    lines
        .iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_default()
        .to_owned()
}

fn marker(flag: &str) -> &'static str {
    match flag {
        "gcc" => "GCC",
        "llvm" => "clang",
        "msvc" => "msvc ",
        _ => "",
    }
}

/// The compiler signatures of the files, read from each file's bytes.
pub fn observe(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .filter_map(|file| std::fs::read(file).ok())
        .flat_map(|bytes| dx_binfmt::producers(&bytes))
        .collect()
}

fn line(
    facts: &Value,
    row: &Value,
    run: &Run,
    lockfile_sha256: &str,
    observe: &dyn Fn(&[PathBuf]) -> Vec<String>,
) -> Result<String, String> {
    let name = text(&row["name"]);
    let identity = &row["identity"];
    let flag = text(&identity["compiler"]);
    let version = text(&identity["version"]);
    let probe = match identity["platform_probes"][run.platform].as_str() {
        Some(probe) => probe,
        None => text(&identity["probe"]),
    };

    let mut tests = Map::new();
    let (mut ran, mut failed) = (0, false);
    for label in labels(row, &["ci_tests", "consumer_tests"]) {
        let outcome = outcome_of(run.testlogs, label);
        ran += usize::from(outcome != "skipped");
        failed |= outcome == "failed";
        tests.insert(label.to_owned(), json!(outcome));
    }
    let total = labels(row, &["ci_tests", "consumer_tests"]).len();

    let mut probe_outcome = "";
    let mut probe_fields = Vec::new();
    if !probe.is_empty() {
        probe_outcome = outcome_of(run.testlogs, probe);
        failed |= probe_outcome == "failed";
        let manifest = output(run.bin, probe, "_starlark_manifest.json");
        if let Ok(found) = std::fs::read_to_string(&manifest) {
            let parsed: Value = serde_json::from_str(&found)
                .map_err(|error| format!("parse {}: {error}", manifest.display()))?;
            probe_lines(&parsed, &mut probe_fields);
        }
    }

    let files: Vec<PathBuf> = labels(row, &["ci_tests"])
        .iter()
        .flat_map(|label| ["", ".exe"].map(|suffix| output(run.bin, label, suffix)))
        .filter(|file| file.is_file())
        .collect();
    let mut observed = if files.is_empty() {
        Vec::new()
    } else {
        observe(&files)
    };
    observed.sort();
    observed.dedup();
    let matched = observed
        .iter()
        .any(|found| found.contains(marker(flag)) && found.contains(version));

    let mut outcome = if failed {
        "failed"
    } else if ran > 0 && (probe.is_empty() || probe_outcome == "passed") {
        "passed"
    } else {
        "skipped"
    };
    if outcome == "passed" && !observed.is_empty() && !matched {
        outcome = "mismatch";
    }
    if flag == "msvc" && outcome == "passed" && !files.is_empty() && observed.is_empty() {
        outcome = "mismatch";
    }
    let blocker = text(&facts["compiler_blockers"][name][run.platform]);
    let tested_outcome = outcome;
    if !blocker.is_empty() && (outcome == "passed" || outcome == "skipped") {
        outcome = "blocked";
    }

    let fields: [(&str, Value); 20] = [
        ("platform", json!(run.platform)),
        ("row", json!(name)),
        ("compiler_flag", json!(flag)),
        ("version", json!(version)),
        ("sha", json!(run.sha)),
        ("compiler", json!(probe_field(&probe_fields, "compiler"))),
        ("target", json!(probe_field(&probe_fields, "target"))),
        ("bazel", json!(run.bazel)),
        ("command", json!(run.command)),
        ("lockfile_sha256", json!(lockfile_sha256)),
        ("job", json!(run.job)),
        ("probe", json!(probe)),
        ("probe_outcome", json!(probe_outcome)),
        ("tests", Value::Object(tests)),
        ("ran", json!(ran)),
        ("total", json!(total)),
        ("observed_producers", json!(observed)),
        ("outcome", json!(outcome)),
        ("tested_outcome", json!(tested_outcome)),
        ("blocker", json!(blocker)),
    ];
    let body: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{}:{value}", json!(key)))
        .collect();
    Ok(format!("{{{}}}", body.join(",")))
}

/// One evidence line per row of `facts` whose section is `compilers`.
pub fn lines(
    facts: &Value,
    run: &Run,
    observe: &dyn Fn(&[PathBuf]) -> Vec<String>,
) -> Result<Vec<String>, String> {
    let lockfile_sha256 = if run.lockfile.is_file() {
        dx_testing::sha256_file(run.lockfile)?
    } else {
        String::new()
    };
    facts["rows"]
        .as_array()
        .ok_or("facts hold no rows")?
        .iter()
        .filter(|row| row["section"] == "compilers")
        .map(|row| line(facts, row, run, &lockfile_sha256, observe))
        .collect()
}

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn var_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

/// Appends the evidence lines to OUT. `args` is OUT PLATFORM FACTS.
pub fn append(args: &[String]) -> Result<(), String> {
    let [out, platform, facts] = args else {
        return Err("usage: producers_bin --evidence OUT PLATFORM FACTS".to_owned());
    };
    let facts: Value = std::fs::read_to_string(facts)
        .map_err(|error| format!("read {facts}: {error}"))
        .and_then(|found| {
            serde_json::from_str(&found).map_err(|error| format!("parse {facts}: {error}"))
        })?;
    let bazel = std::env::var("DX_BAZEL_VERSION").map_err(|_| "DX_BAZEL_VERSION is not set")?;
    let (bin, testlogs) = (
        var_or("DX_BIN", "bazel-bin"),
        var_or("DX_TESTLOGS", "bazel-testlogs"),
    );
    let lockfile = var_or("DX_LOCKFILE", "MODULE.bazel.lock");
    let job = format!(
        "{}/{}/actions/runs/{}",
        var("GITHUB_SERVER_URL"),
        var("GITHUB_REPOSITORY"),
        var("GITHUB_RUN_ID")
    );
    let run = Run {
        platform,
        bin: Path::new(&bin),
        testlogs: Path::new(&testlogs),
        lockfile: Path::new(&lockfile),
        bazel: bazel.trim_end(),
        command: &var("DX_EVIDENCE_COMMAND"),
        sha: &var("GITHUB_SHA"),
        job: &job,
    };
    let found = lines(&facts, &run, &observe)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out)
        .map_err(|error| format!("open {out}: {error}"))?;
    for line in found {
        writeln!(file, "{line}").map_err(|error| format!("write {out}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Run, lines};
    use dx_test_scratch::{scratch, write};
    use serde_json::{Value, json};
    use std::path::Path;

    fn facts() -> Value {
        json!({"rows": [
            {"name": "cc_a", "section": "compilers", "ci_tests": ["//p:a1", "//p:a2"], "consumer_tests": ["//q:a3"],
             "identity": {"compiler": "a", "version": "1.2.3", "probe": "//p:probe_a",
                          "platform_probes": {"linux_arm64": "//p:probe_a_arm64"}}},
            {"name": "cc_b", "section": "compilers", "ci_tests": ["//p:b1"], "consumer_tests": [],
             "identity": {"compiler": "b", "version": "", "probe": ""}},
            {"name": "cc_m", "section": "compilers", "ci_tests": ["//p:m1"], "consumer_tests": [],
             "identity": {"compiler": "msvc", "version": "35728", "probe": ""}},
            {"name": "other", "section": "languages", "ci_tests": ["//p:o"], "consumer_tests": [], "identity": {}},
        ],
        "compiler_blockers": {"cc_a": {"windows_arm64": "waiting on a runner"},
                              "cc_m": {"windows_arm64": "waiting on a runner"}}})
    }

    fn log(root: &Path, label: &str, xml: &str) {
        let path = label.trim_start_matches("//").replace(':', "/");
        write(root, &format!("logs/{path}/test.xml"), xml);
    }

    fn passed(root: &Path, label: &str) {
        log(root, label, "<testsuites><testcase/></testsuites>");
    }

    fn failed(root: &Path, label: &str) {
        log(
            root,
            label,
            "<testsuites><testcase><failure/></testcase></testsuites>",
        );
    }

    fn run_with(root: &Path, platform: &str, command: &str, signatures: &[&str]) -> Vec<Value> {
        let run = Run {
            platform,
            bin: &root.join("bin"),
            testlogs: &root.join("logs"),
            lockfile: &root.join("MODULE.bazel.lock"),
            bazel: "bazel 9",
            command,
            sha: "",
            job: "//actions/runs/",
        };
        let observe =
            |_: &[std::path::PathBuf]| signatures.iter().map(|found| (*found).to_owned()).collect();
        lines(&facts(), &run, &observe)
            .expect("evidence lines")
            .iter()
            .map(|line| serde_json::from_str(line).expect("json line"))
            .collect()
    }

    fn run_on(root: &Path, platform: &str) -> Vec<Value> {
        run_with(root, platform, "", &[])
    }

    fn run(root: &Path) -> Vec<Value> {
        run_on(root, "linux_x86_64")
    }

    fn row<'a>(lines: &'a [Value], name: &str) -> &'a Value {
        lines
            .iter()
            .find(|line| line["row"] == name)
            .unwrap_or_else(|| panic!("no line for {name}"))
    }

    #[test]
    fn all_passed() {
        let dir = scratch("compiler_evidence");
        for label in ["//p:probe_a", "//p:a1", "//p:a2", "//q:a3", "//p:b1"] {
            passed(dir.path(), label);
        }
        let lines = run(dir.path());
        assert_eq!(lines.len(), 3);
        let a = row(&lines, "cc_a");
        assert_eq!(a["outcome"], "passed");
        assert_eq!((a["ran"].as_u64(), a["total"].as_u64()), (Some(3), Some(3)));
        assert_eq!(a["tests"]["//p:a1"], "passed");
    }

    #[test]
    fn one_failed() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:probe_a");
        passed(dir.path(), "//p:a1");
        failed(dir.path(), "//p:a2");
        let a = &run(dir.path())[0];
        assert_eq!(a["outcome"], "failed");
        assert_eq!(a["tests"]["//p:a2"], "failed");
        assert_eq!(a["tests"]["//q:a3"], "skipped");
    }

    #[test]
    fn an_error_element_is_a_failure() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:probe_a");
        log(
            dir.path(),
            "//p:a1",
            "<testsuites><testcase><error/></testcase></testsuites>",
        );
        assert_eq!(run(dir.path())[0]["outcome"], "failed");
    }

    #[test]
    fn none_ran_is_skipped() {
        let dir = scratch("compiler_evidence");
        let lines = run(dir.path());
        let a = row(&lines, "cc_a");
        assert_eq!(a["outcome"], "skipped");
        assert_eq!((a["ran"].as_u64(), a["total"].as_u64()), (Some(0), Some(3)));
    }

    #[test]
    fn skipped_probe_is_not_passed() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:a1");
        let lines = run(dir.path());
        assert_eq!(row(&lines, "cc_a")["outcome"], "skipped");
    }

    #[test]
    fn platform_probe_replaces_the_probe_on_its_platform() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:probe_a_arm64");
        passed(dir.path(), "//p:a1");
        let lines = run_on(dir.path(), "linux_arm64");
        let a = row(&lines, "cc_a");
        assert_eq!(a["probe"], "//p:probe_a_arm64");
        assert_eq!(a["probe_outcome"], "passed");
        assert_eq!(a["outcome"], "passed");
    }

    #[test]
    fn other_platforms_keep_the_default_probe() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:probe_a_arm64");
        passed(dir.path(), "//p:a1");
        let lines = run(dir.path());
        let a = row(&lines, "cc_a");
        assert_eq!(a["probe"], "//p:probe_a");
        assert_eq!(a["outcome"], "skipped");
    }

    #[test]
    fn row_without_probe_passes_on_its_tests() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:b1");
        let lines = run(dir.path());
        let b = row(&lines, "cc_b");
        assert_eq!(b["outcome"], "passed");
        assert_eq!(b["probe"], "");
        assert_eq!(b["version"], "");
    }

    #[test]
    fn version_is_copied_through() {
        let dir = scratch("compiler_evidence");
        let lines = run(dir.path());
        let a = row(&lines, "cc_a");
        assert_eq!(a["version"], "1.2.3");
        assert_eq!(a["compiler_flag"], "a");
        assert_eq!(a["bazel"], "bazel 9");
    }

    #[test]
    fn probe_manifest_fields_are_recorded() {
        let dir = scratch("compiler_evidence");
        write(
            dir.path(),
            "bin/p/probe_a_starlark_manifest.json",
            &json!({"checks": ["other", "field cc.compiler=gcc 1.2.3\nfield cc.target=x86_64-linux"]}).to_string(),
        );
        let lines = run(dir.path());
        let a = row(&lines, "cc_a");
        assert_eq!(a["compiler"], "gcc 1.2.3");
        assert_eq!(a["target"], "x86_64-linux");
        assert_eq!(row(&lines, "cc_b")["compiler"], "");
    }

    #[test]
    fn command_and_lockfile_digest_are_recorded() {
        let dir = scratch("compiler_evidence");
        write(dir.path(), "MODULE.bazel.lock", "lock\n");
        let lines = run_with(dir.path(), "linux_x86_64", "bazel test //p:a1", &[]);
        let a = row(&lines, "cc_a");
        assert_eq!(a["command"], "bazel test //p:a1");
        assert_eq!(
            a["lockfile_sha256"],
            dx_testing::sha256_file(&dir.path().join("MODULE.bazel.lock")).expect("digest")
        );
    }

    #[test]
    fn missing_lockfile_leaves_the_digest_empty() {
        let dir = scratch("compiler_evidence");
        let lines = run(dir.path());
        assert_eq!(row(&lines, "cc_a")["lockfile_sha256"], "");
        assert_eq!(row(&lines, "cc_a")["command"], "");
    }

    fn observe(root: &Path, signatures: &[&str]) -> Vec<Value> {
        write(root, "bin/p/a1", "binary");
        observe_with(root, signatures)
    }

    fn observe_with(root: &Path, signatures: &[&str]) -> Vec<Value> {
        for label in ["//p:probe_a", "//p:a1", "//p:a2", "//q:a3", "//p:m1"] {
            passed(root, label);
        }
        run_with(root, "linux_x86_64", "", signatures)
    }

    #[test]
    fn observed_producer_with_the_version_passes() {
        let dir = scratch("compiler_evidence");
        let a = &observe(
            dir.path(),
            &["GCC: (GNU) 9.9.9", "GCC: (GNU) 1.2.3", "GCC: (GNU) 9.9.9"],
        )[0];
        assert_eq!(a["outcome"], "passed");
        assert_eq!(
            a["observed_producers"],
            json!(["GCC: (GNU) 1.2.3", "GCC: (GNU) 9.9.9"])
        );
    }

    #[test]
    fn observed_producer_with_another_version_is_a_mismatch() {
        let dir = scratch("compiler_evidence");
        let a = &observe(dir.path(), &["GCC: (GNU) 9.9.9"])[0];
        assert_eq!(a["outcome"], "mismatch");
    }

    #[test]
    fn no_observed_producer_keeps_the_outcome() {
        let dir = scratch("compiler_evidence");
        let a = &observe(dir.path(), &[])[0];
        assert_eq!(a["outcome"], "passed");
        assert_eq!(a["observed_producers"], json!([]));
    }

    #[test]
    fn a_row_without_executables_is_not_observed() {
        let dir = scratch("compiler_evidence");
        let lines = observe_with(dir.path(), &["GCC: (GNU) 9.9.9"]);
        let a = row(&lines, "cc_a");
        assert_eq!(a["outcome"], "passed");
        assert_eq!(a["observed_producers"], json!([]));
    }

    #[test]
    fn msvc_rich_producer_passes() {
        let dir = scratch("compiler_evidence");
        write(dir.path(), "bin/p/m1.exe", "binary");
        let lines = observe_with(dir.path(), &["msvc 261.35728"]);
        assert_eq!(row(&lines, "cc_m")["outcome"], "passed");
    }

    #[test]
    fn msvc_rich_producer_of_another_build_is_a_mismatch() {
        let dir = scratch("compiler_evidence");
        write(dir.path(), "bin/p/m1.exe", "binary");
        let lines = observe_with(dir.path(), &["msvc 261.33145"]);
        assert_eq!(row(&lines, "cc_m")["outcome"], "mismatch");
    }

    #[test]
    fn blocked_platform_keeps_the_tested_outcome() {
        let dir = scratch("compiler_evidence");
        for label in ["//p:probe_a", "//p:a1", "//p:b1"] {
            passed(dir.path(), label);
        }
        let lines = run_on(dir.path(), "windows_arm64");
        let a = row(&lines, "cc_a");
        assert_eq!(a["outcome"], "blocked");
        assert_eq!(a["tested_outcome"], "passed");
        assert_eq!(a["blocker"], "waiting on a runner");
        let m = row(&lines, "cc_m");
        assert_eq!(m["outcome"], "blocked");
        assert_eq!(m["tested_outcome"], "skipped");
        let b = row(&lines, "cc_b");
        assert_eq!(b["outcome"], "passed");
        assert_eq!(b["blocker"], "");
    }

    #[test]
    fn blocked_platform_still_reports_failures() {
        let dir = scratch("compiler_evidence");
        failed(dir.path(), "//p:a1");
        let lines = run_on(dir.path(), "windows_arm64");
        let a = row(&lines, "cc_a");
        assert_eq!(a["outcome"], "failed");
        assert_eq!(a["blocker"], "waiting on a runner");
    }

    #[test]
    fn blocker_on_another_platform_does_not_apply() {
        let dir = scratch("compiler_evidence");
        passed(dir.path(), "//p:probe_a");
        passed(dir.path(), "//p:a1");
        let a = &run(dir.path())[0];
        assert_eq!(a["outcome"], "passed");
        assert_eq!(a["blocker"], "");
    }

    #[test]
    fn msvc_binary_without_rich_header_is_a_mismatch() {
        let dir = scratch("compiler_evidence");
        write(dir.path(), "bin/p/m1.exe", "binary");
        let lines = observe_with(dir.path(), &[]);
        let m = row(&lines, "cc_m");
        assert_eq!(m["outcome"], "mismatch");
        assert_eq!(m["observed_producers"], json!([]));
    }

    #[test]
    fn msvc_row_without_binaries_keeps_outcome() {
        let dir = scratch("compiler_evidence");
        let lines = observe_with(dir.path(), &[]);
        assert_eq!(row(&lines, "cc_m")["outcome"], "passed");
    }
}
