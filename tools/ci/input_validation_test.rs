use std::collections::BTreeMap;
use std::process::Command;

use dx_testing::{read_runfiles, runfiles_root};
use input_validation::{CHECKS, MATRIX_CHECKS, PLATFORMS};
use serde_json::Value;

const FIXTURE_DIR: &str = "tools/ci/input_validation_fixtures";
const CI_WORKFLOW: &str = ".github/workflows/ci.yml";
const RELEASE_WORKFLOW: &str = ".github/workflows/release-cells.yml";
const CONSUMER_WORKFLOW: &str = ".github/workflows/release-consumer.yml";
const EVIDENCE_WORKFLOW: &str = ".github/workflows/evidence.yml";
const ACTIONLINT_CONFIG: &str = ".github/actionlint.yaml";
const DOCS: &str = "docs/github-ci.md";
const CALLERS: [&str; 2] = ["examples/consumer-ci/caller.yml", CI_WORKFLOW];
const INPUTS: [&str; 6] = [
    "DX_PLATFORMS",
    "DX_DISABLED_CHECKS",
    "DX_MIN_COVERAGE",
    "DX_MIN_CHANGED_LINE_COVERAGE",
    "DX_INVOCATION_ID",
    "DX_FUZZ_TIME",
];
const PLAIN_OUTPUTS: [&str; 8] = [
    "artifact_suffix",
    "cache_saver",
    "check_lines",
    "checks_timeout",
    "disabled",
    "min_changed_line_coverage",
    "min_coverage",
    "snapshot_artifact",
];

struct Expect {
    inputs: Vec<(String, String)>,
    status: Option<i32>,
    says: Vec<String>,
    outputs: BTreeMap<String, String>,
}

fn plain(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

fn expectation(case: &str) -> Expect {
    let text = read_runfiles(&format!("{FIXTURE_DIR}/{case}/expect.txt"));
    let mut expect = Expect {
        inputs: Vec::new(),
        status: None,
        says: Vec::new(),
        outputs: BTreeMap::new(),
    };
    for line in text.lines() {
        if let Some(json) = line.strip_prefix("input ") {
            let parsed: Value = serde_json::from_str(json).expect("fixture input JSON");
            expect.inputs = parsed
                .as_object()
                .expect("fixture input object")
                .iter()
                .map(|(key, value)| (key.clone(), plain(value)))
                .collect();
        } else if let Some(status) = line.strip_prefix("exit ") {
            expect.status = status.parse().ok();
        } else if let Some(says) = line.strip_prefix("says ") {
            expect.says.push(says.to_string());
        } else if let Some(out) = line.strip_prefix("out ") {
            let (key, value) = out.split_once(' ').expect("fixture output value");
            let parsed: Value = serde_json::from_str(value).expect("fixture output JSON");
            let value = if PLAIN_OUTPUTS.contains(&key) {
                plain(&parsed)
            } else {
                value.to_string()
            };
            expect.outputs.insert(key.to_string(), value);
        }
    }
    expect
}

fn cases() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(runfiles_root().join(FIXTURE_DIR))
        .expect("fixture directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn run(
    scratch: &std::path::Path,
    case: &str,
    expect: &Expect,
) -> (i32, String, BTreeMap<String, String>) {
    let output = scratch.join(format!("{case}.outputs"));
    let bin =
        runfiles_root().join(std::env::var("DX_INPUT_VALIDATION").expect("DX_INPUT_VALIDATION"));
    let mut command = Command::new(bin);
    for name in INPUTS {
        command.env_remove(name);
    }
    for (key, value) in &expect.inputs {
        command.env(key, value);
    }
    let done = command
        .env("GITHUB_OUTPUT", &output)
        .output()
        .expect("run input_validation_bin");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    );
    let mut written = BTreeMap::new();
    let mut lines = std::fs::read_to_string(&output)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter();
    while let Some(line) = lines.next() {
        if let Some((key, _)) = line.split_once("<<DX_EOF") {
            let body: Vec<String> = lines.by_ref().take_while(|l| l != "DX_EOF").collect();
            written.insert(key.to_string(), body.join("\n"));
        } else if let Some((key, value)) = line.split_once('=') {
            written.insert(key.to_string(), value.to_string());
        }
    }
    (done.status.code().unwrap_or(-1), text, written)
}

#[test]
fn every_fixture_states_one_expected_outcome() {
    let names = cases();
    assert!(!names.is_empty(), "{FIXTURE_DIR} holds no fixture");
    for case in names {
        let expect = expectation(&case);
        assert!(
            matches!(expect.status, Some(0 | 1)),
            "{case} declares no exit status"
        );
        assert!(!expect.says.is_empty(), "{case} expects no validator text");
        assert_eq!(
            expect.status == Some(0),
            !expect.outputs.is_empty(),
            "{case} pairs its status with its outputs"
        );
        for (name, _) in &expect.inputs {
            assert!(
                INPUTS.contains(&name.as_str()),
                "{case} passes unread input {name}"
            );
        }
    }
}

#[test]
fn fixtures_report_the_expected_diagnostic_and_write_the_validated_outputs() {
    let scratch = dx_test_scratch::scratch("input-validation");
    for case in cases() {
        let expect = expectation(&case);
        let (code, text, written) = run(scratch.path(), &case, &expect);
        assert_eq!(Some(code), expect.status, "{case} exited {code}: {text}");
        for says in &expect.says {
            assert!(
                text.contains(says.as_str()),
                "{case} never reported [{says}]: {text}"
            );
        }
        if expect.status == Some(0) {
            assert_eq!(
                written, expect.outputs,
                "{case} wrote other outputs: {text}"
            );
        } else {
            assert!(
                written.is_empty(),
                "{case} wrote {written:?} beside its rejection"
            );
        }
    }
}

#[test]
fn every_input_has_a_rejected_case() {
    let mut rejected = Vec::new();
    for case in cases() {
        let expect = expectation(&case);
        if expect.status == Some(1) {
            rejected.extend(expect.inputs.into_iter().map(|(name, _)| name));
        }
    }
    let missing: Vec<&str> = INPUTS
        .into_iter()
        .filter(|name| !rejected.iter().any(|seen| seen == name))
        .collect();
    assert!(missing.is_empty(), "no fixture rejects {missing:?}");
}

#[test]
fn matrix_checks_are_unique_gated_ids() {
    let ids: Vec<&str> = MATRIX_CHECKS.iter().map(|row| row.id).collect();
    for (index, id) in ids.iter().enumerate() {
        assert!(!ids[..index].contains(id), "MATRIX_CHECKS repeats {id}");
        assert!(CHECKS.contains(id), "MATRIX_CHECKS names ungated {id}");
        assert!(
            !["security-audit", "coverage"].contains(id),
            "{id} is not a matrix check"
        );
    }
    for row in &MATRIX_CHECKS {
        assert!(
            !row.args.is_empty() && row.args.iter().all(|arg| !arg.contains(' ')),
            "{}",
            row.id
        );
        assert!(row.timeout > 0, "{} has no timeout", row.id);
        for token in row.reports {
            let (kind, file) = token.split_once('=').expect("report kind=file");
            let word = |text: &str, extra: &str| {
                !text.is_empty()
                    && text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
            };
            assert!(
                kind.chars().all(|c| c.is_ascii_lowercase()) && word(file, "_.-"),
                "{} has report {token}",
                row.id
            );
        }
    }
}

fn table() -> Vec<(String, String)> {
    PLATFORMS
        .iter()
        .map(|(name, runner)| (name.to_string(), runner.to_string()))
        .collect()
}

fn dict_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line == "PLATFORMS = {" {
            inside = true;
        } else if line == "}" {
            inside = false;
        } else if inside {
            if let Some((key, value)) = line.trim_end_matches(',').split_once("\": ") {
                pairs.push((
                    key.trim_matches('"').to_string(),
                    value.trim_matches('"').to_string(),
                ));
            }
        }
    }
    pairs
}

fn preflight_pairs(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter_map(|pair| {
            let platform = pair[0].strip_prefix("- platform: ")?;
            let runner = pair[1].strip_prefix("runner: ")?;
            Some((platform.to_string(), runner.to_string()))
        })
        .collect()
}

fn dogfood_platforms(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("platforms: '"))
        .filter_map(|rest| rest.strip_suffix('\''))
        .collect();
    match lines.as_slice() {
        [one] => serde_json::from_str(one).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn runners_of(text: &str) -> Vec<String> {
    let mut runners: Vec<String> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("runner: "))
        .map(str::to_string)
        .collect();
    runners.sort();
    runners.dedup();
    runners
}

fn actionlint_labels(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(str::to_string)
        .collect()
}

fn platform_drift(ci: &str, release: &str, consumer: &str, actionlint: &str) -> Vec<String> {
    let table = table();
    let names: Vec<String> = table.iter().map(|(name, _)| name.clone()).collect();
    let mut want: Vec<String> = table.iter().map(|(_, runner)| runner.clone()).collect();
    want.sort();
    let mut problems = Vec::new();
    let pairs = preflight_pairs(ci);
    if pairs.is_empty()
        || !pairs.len().is_multiple_of(table.len())
        || pairs.chunks(table.len()).any(|chunk| chunk != table)
    {
        problems.push(format!("{CI_WORKFLOW} preflight matrix drifted"));
    }
    if dogfood_platforms(ci) != names {
        problems.push(format!("{CI_WORKFLOW} dogfood platforms drifted"));
    }
    if runners_of(release) != want {
        problems.push(format!("{RELEASE_WORKFLOW} runners drifted"));
    }
    if runners_of(consumer) != want {
        problems.push(format!("{CONSUMER_WORKFLOW} runners drifted"));
    }
    let labels = actionlint_labels(actionlint);
    if !want.iter().all(|runner| labels.contains(runner)) {
        problems.push(format!("{ACTIONLINT_CONFIG} labels miss a runner"));
    }
    problems
}

fn copies() -> (String, String, String, String) {
    (
        read_runfiles(CI_WORKFLOW),
        read_runfiles(RELEASE_WORKFLOW),
        read_runfiles(CONSUMER_WORKFLOW),
        read_runfiles(ACTIONLINT_CONFIG),
    )
}

#[test]
fn platform_copies_match_the_table() {
    let (ci, release, consumer, actionlint) = copies();
    assert_eq!(
        platform_drift(&ci, &release, &consumer, &actionlint),
        Vec::<String>::new()
    );
}

#[test]
fn platform_parity_catches_a_drifted_copy() {
    let (ci, release, consumer, actionlint) = copies();
    let dropped = ci.replace(
        "- platform: windows_arm64\n            runner: windows-11-arm\n",
        "",
    );
    assert!(platform_drift(&dropped, &release, &consumer, &actionlint)
        .iter()
        .any(|problem| problem.contains(CI_WORKFLOW)));
    let swapped = ci.replace("runner: macos-15", "runner: macos-14");
    assert!(platform_drift(&swapped, &release, &consumer, &actionlint)
        .iter()
        .any(|problem| problem.contains("preflight")));
    let dogfood = ci.replace("\"windows_x86_64\", ", "");
    assert!(platform_drift(&dogfood, &release, &consumer, &actionlint)
        .iter()
        .any(|problem| problem.contains("dogfood")));
    let drifted = release.replace("macos-15", "macos-14");
    assert!(platform_drift(&ci, &drifted, &consumer, &actionlint)
        .iter()
        .any(|problem| problem.contains(RELEASE_WORKFLOW)));
    let drifted = consumer.replace("macos-15", "macos-14");
    assert!(platform_drift(&ci, &release, &drifted, &actionlint)
        .iter()
        .any(|problem| problem.contains(CONSUMER_WORKFLOW)));
    let actionlint = actionlint.replace("windows-11-arm", "x");
    assert!(platform_drift(&ci, &release, &consumer, &actionlint)
        .iter()
        .any(|problem| problem.contains(ACTIONLINT_CONFIG)));
}

#[test]
fn evidence_runners_match_the_table() {
    assert_eq!(
        dict_pairs(&read_runfiles(EVIDENCE_WORKFLOW)),
        table(),
        "{EVIDENCE_WORKFLOW} PLATFORMS drifted from input_validation.rs"
    );
}

#[test]
fn docs_name_every_platform_and_gated_check() {
    assert_eq!(PLATFORMS.len(), 5);
    assert_eq!(CHECKS.len(), 9);
    let page = read_runfiles(DOCS);
    let missing: Vec<&str> = PLATFORMS
        .iter()
        .map(|(name, _)| *name)
        .chain(CHECKS)
        .filter(|name| !page.contains(&format!("`{name}`")))
        .collect();
    assert!(missing.is_empty(), "{DOCS} never names {missing:?}");
}

#[test]
fn callers_request_only_mapped_platforms() {
    for caller in CALLERS {
        let text = read_runfiles(caller);
        let values: Vec<&str> = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("platforms:"))
            .map(str::trim)
            .collect();
        assert_eq!(
            values.len(),
            1,
            "{caller} sets platforms {} times",
            values.len()
        );
        let json = values[0]
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
            .unwrap_or_else(|| panic!("{caller} platforms must be a quoted JSON array"));
        let requested: Vec<String> = serde_json::from_str(json).expect("platforms JSON array");
        assert!(!requested.is_empty(), "{caller} requests no platforms");
        for label in requested {
            assert!(
                PLATFORMS.iter().any(|(name, _)| *name == label),
                "{caller} platform {label} is not in the table"
            );
        }
    }
}

#[test]
fn deployables_accept_empty_and_label_lists() {
    use input_validation::validate_deployables;
    assert_eq!(validate_deployables("").unwrap(), Vec::<String>::new());
    assert_eq!(
        validate_deployables(r#"["//pkg:npm", "@repo//pkg:github"]"#).unwrap(),
        vec!["//pkg:npm".to_string(), "@repo//pkg:github".to_string()]
    );
}

#[test]
fn deployables_reject_bad_input() {
    use input_validation::validate_deployables;
    for (text, says) in [
        ("[]", "empty array"),
        ("{}", "not an array"),
        ("//pkg:npm", "not a JSON array"),
        ("[1]", "not a label string"),
        (r#"["pkg:npm"]"#, "not a Bazel label"),
        (r#"["//pkg"]"#, "not a Bazel label"),
        (r#"["//pkg:a b"]"#, "not a Bazel label"),
        (r#"["//pkg:a","//pkg:a"]"#, "twice"),
    ] {
        let error = validate_deployables(text).unwrap_err();
        assert!(error.contains(says), "{text}: {error}");
    }
}

#[test]
fn deployables_mode_prints_labels_and_rejects_bad_lists() {
    let bin =
        runfiles_root().join(std::env::var("DX_INPUT_VALIDATION").expect("DX_INPUT_VALIDATION"));
    let run = |text: &str| {
        Command::new(&bin)
            .arg("deployables")
            .env("DX_DEPLOYABLES", text)
            .output()
            .expect("run input_validation_bin")
    };
    let good = run(r#"["//pkg:npm", "//pkg:gh"]"#);
    assert!(good.status.success());
    assert_eq!(String::from_utf8_lossy(&good.stdout), "//pkg:npm\n//pkg:gh\n");
    let bad = run("[]");
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("empty array"));
}
