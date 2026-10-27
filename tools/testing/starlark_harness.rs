//! Evaluates one starlark_test manifest without host shell tooling.
//!
//! The Starlark rule writes one JSON manifest per case and runs this binary as
//! the test executable. The harness resolves every input through its own
//! runfiles, evaluates the analysis-time assertions Starlark already decided,
//! checks the required file substrings byte for byte, prints the analysis
//! observations, and renders one PASS or FAIL line per assertion. No sh, awk,
//! grep, cat, or batch interpreter is launched.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};

use dx_path::Resolver;

/// Names the environment entry carrying the manifest runfiles key.
pub const MANIFEST_ENV: &str = "STARLARK_HARNESS_MANIFEST";
/// Accepts only manifests written for this evaluator.
pub const MANIFEST_VERSION: u64 = 2;

/// The rendered outcome of one manifest evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Every PASS, FAIL, and observation line plus the summary line, in order.
    pub lines: Vec<String>,
    /// The number of passing assertions.
    pub passed: usize,
    /// The number of failing assertions.
    pub failed: usize,
}

impl Report {
    /// Renders the whole report as test output text.
    pub fn text(&self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }

    /// Returns whether every assertion passed.
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

/// Reads one required string field from a manifest object.
fn field(value: &serde_json::Value, path: &str, name: &str) -> std::io::Result<String> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| bad(format!("{path} has no string field {name}")))
}

/// Reads one required string array field from a manifest object.
fn string_list(value: &serde_json::Value, path: &str, name: &str) -> std::io::Result<Vec<String>> {
    let list = value
        .get(name)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bad(format!("{path} has no string array field {name}")))?;
    let mut out = Vec::with_capacity(list.len());
    for entry in list {
        let text = entry
            .as_str()
            .ok_or_else(|| bad(format!("{path} field {name} holds a non-string entry")))?;
        out.push(text.to_owned());
    }
    Ok(out)
}

/// Evaluates one equality assertion.
fn evaluate_equal(name: &str, expected: &str, actual: &str, report: &mut Report) {
    if expected == actual {
        pass(report, format!("PASS: {name}"));
    } else {
        fail(
            report,
            format!("FAIL: {name}\n  expected: {expected}\n  actual:   {actual}"),
        );
    }
}

/// Evaluates one boolean assertion.
fn evaluate_bool(name: &str, want: &str, actual: &str, passed: &str, report: &mut Report) {
    if passed == "True" {
        pass(report, format!("PASS: {name}"));
    } else {
        fail(
            report,
            format!("FAIL: {name}\n  expected: {want}\n  actual:   {actual}"),
        );
    }
}

/// Evaluates one membership assertion.
fn evaluate_contains(name: &str, haystack: &str, needle: &str, passed: &str, report: &mut Report) {
    if passed == "True" {
        pass(report, format!("PASS: {name}"));
    } else {
        fail(
            report,
            format!("FAIL: {name}\n  haystack: {haystack}\n  missing:  {needle}"),
        );
    }
}

/// Evaluates one substring assertion.
fn evaluate_match(name: &str, value: &str, want: &str, passed: &str, report: &mut Report) {
    if passed == "True" {
        pass(report, format!("PASS: {name}"));
    } else {
        fail(
            report,
            format!("FAIL: {name}\n  value: {value}\n  missing substring: {want}"),
        );
    }
}

/// Evaluates one decoded check record in declaration order.
fn evaluate_check(check: &serde_json::Value, report: &mut Report) -> std::io::Result<()> {
    let kind = field(check, "check", "kind")?;
    let name = field(check, "check", "name")?;
    match kind.as_str() {
        "equal" => {
            let expected = field(check, "check", "expected")?;
            let actual = field(check, "check", "actual")?;
            evaluate_equal(&name, &expected, &actual, report);
        }
        "true" => {
            let actual = field(check, "check", "actual")?;
            let passed = field(check, "check", "passed")?;
            evaluate_bool(&name, "True", &actual, &passed, report);
        }
        "false" => {
            let actual = field(check, "check", "actual")?;
            let passed = field(check, "check", "passed")?;
            evaluate_bool(&name, "False", &actual, &passed, report);
        }
        "contains" => {
            let haystack = field(check, "check", "haystack")?;
            let needle = field(check, "check", "needle")?;
            let passed = field(check, "check", "passed")?;
            evaluate_contains(&name, &haystack, &needle, &passed, report);
        }
        "match" => {
            let value = field(check, "check", "value")?;
            let want = field(check, "check", "want")?;
            let passed = field(check, "check", "passed")?;
            evaluate_match(&name, &value, &want, &passed, report);
        }
        other => {
            return Err(bad(format!("check {name} has unknown kind {other}")));
        }
    }
    Ok(())
}

/// Returns whether the needle bytes occur anywhere in the haystack bytes.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Returns the length of the longest prefix of the needle that occurs in the haystack.
fn longest_prefix_match(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut failure = vec![0usize; needle.len()];
    let mut k = 0;
    for i in 1..needle.len() {
        while k > 0 && needle[i] != needle[k] {
            k = failure[k - 1];
        }
        if needle[i] == needle[k] {
            k += 1;
        }
        failure[i] = k;
    }
    let mut best = 0;
    let mut matched = 0;
    for &byte in haystack {
        while matched > 0 && byte != needle[matched] {
            matched = failure[matched - 1];
        }
        if byte == needle[matched] {
            matched += 1;
        }
        best = best.max(matched);
        if matched == needle.len() {
            break;
        }
    }
    best
}

/// Returns the 0-based index and text of the first needle line that breaks the match.
///
/// A line is missing when the needle stops matching inside it. A line that does not
/// directly follow its predecessor in the haystack is reported as out of order.
fn first_missing_line<'a>(haystack: &[u8], needle: &'a str) -> (usize, &'a str) {
    let bytes = needle.as_bytes();
    let matched = longest_prefix_match(haystack, bytes);
    let mut index = bytes[..matched]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count();
    if matched > 0 && bytes.get(matched) == Some(&b'\n') {
        index += 1;
    }
    (index, needle.split('\n').nth(index).unwrap_or(needle))
}

/// Evaluates one staged file against every required substring.
fn evaluate_file(
    entry: &serde_json::Value,
    resolver: &Resolver,
    report: &mut Report,
) -> std::io::Result<()> {
    let label = field(entry, "file", "label")?;
    let key = field(entry, "file", "key")?;
    let needles = string_list(entry, "file", "needles")?;
    if needles.is_empty() {
        return Err(bad(format!("file {label} names no substrings")));
    }
    let path = resolver
        .lookup_from(&key, "")
        .map_err(|error| bad(format!("cannot resolve runfile {key}: {error}")))?;
    let bytes = std::fs::read(&path)
        .map_err(|error| bad(format!("cannot read {}: {error}", path.display())))?;
    let shown = path.to_string_lossy().into_owned();
    for (index, needle) in needles.iter().enumerate() {
        let ordinal = format!("{}/{}", index + 1, needles.len());
        if contains_bytes(&bytes, needle.as_bytes()) {
            pass(
                report,
                format!("PASS: file {label} contains substring {ordinal}"),
            );
        } else {
            let (line_index, line) = first_missing_line(&bytes, needle);
            let total = needle.split('\n').count();
            let detail = if total > 1 {
                format!("\n  needle line: {} of {total}", line_index + 1)
            } else {
                String::new()
            };
            fail(
                report,
                format!(
                    "FAIL: file {label} is missing substring {ordinal}\n  substring: {line}{detail}\n  file: {shown}"
                ),
            );
        }
    }
    Ok(())
}

/// Evaluates the check records of one parsed manifest.
pub fn evaluate(manifest: &serde_json::Value) -> std::io::Result<Report> {
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| bad("manifest has no numeric field version".to_owned()))?;
    if version != MANIFEST_VERSION {
        return Err(bad(format!(
            "unsupported manifest version {version}: want {MANIFEST_VERSION}"
        )));
    }
    let name = field(manifest, "manifest", "name")?;
    if name.is_empty() {
        return Err(bad("manifest names no test".to_owned()));
    }
    let checks = manifest
        .get("checks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bad("manifest has no array field checks".to_owned()))?;
    let files = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bad("manifest has no array field files".to_owned()))?;
    let observations = string_list(manifest, "manifest", "observations")?;
    if checks.is_empty() && files.is_empty() && observations.is_empty() {
        return Err(bad(format!("manifest for {name} carries no evidence")));
    }
    let mut report = Report {
        lines: Vec::new(),
        passed: 0,
        failed: 0,
    };
    for check in checks {
        evaluate_check(check, &mut report)?;
    }
    Ok(report)
}

/// Evaluates the file entries of one manifest through one resolver.
pub fn evaluate_files(
    manifest: &serde_json::Value,
    resolver: &Resolver,
    report: &mut Report,
) -> std::io::Result<()> {
    let files = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| bad("manifest has no array field files".to_owned()))?;
    for entry in files {
        evaluate_file(entry, resolver, report)?;
    }
    Ok(())
}

/// Appends the analysis observations after the assertion lines.
///
/// Observation lines carry no verdict; they never change the pass or fail
/// counts.
pub fn evaluate_observations(
    manifest: &serde_json::Value,
    report: &mut Report,
) -> std::io::Result<()> {
    let observations = string_list(manifest, "manifest", "observations")?;
    if observations.is_empty() {
        return Ok(());
    }
    report.lines.push("--- observations ---".to_owned());
    report.lines.extend(observations);
    Ok(())
}

/// Records one passing assertion.
fn pass(report: &mut Report, line: String) {
    report.lines.push(line);
    report.passed += 1;
}

/// Records one failing assertion.
fn fail(report: &mut Report, line: String) {
    report.lines.push(line);
    report.failed += 1;
}

/// Appends the closing summary line.
fn summarize(report: &mut Report) {
    let passed = report.passed;
    let failed = report.failed;
    report
        .lines
        .push(format!("starlark_test: {passed} passed, {failed} failed"));
}

/// Returns the invoked test executable without resolving its symlink.
fn invoked_exe() -> std::io::Result<PathBuf> {
    let argv0 = std::env::args()
        .next()
        .ok_or_else(|| bad("no argv[0]".to_owned()))?;
    let path = PathBuf::from(argv0);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| bad(format!("cannot resolve the test executable: {error}")))
}

/// Returns the runfiles tree above one executable path, when there is one.
fn tree_ancestor(exe: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in exe.components() {
        root.push(component.as_os_str());
        if component
            .as_os_str()
            .to_string_lossy()
            .ends_with(".runfiles")
        {
            return Some(root);
        }
    }
    None
}

/// Resolves the runfiles of the invoked test executable.
fn resolver_for(exe: &Path) -> std::io::Result<Resolver> {
    if let Some(tree) = tree_ancestor(exe) {
        if tree.is_dir() {
            return Resolver::for_tree(&tree)
                .map_err(|error| bad(format!("no runfiles tree at {}: {error}", tree.display())));
        }
    }
    Resolver::for_binary(exe)
        .map_err(|error| bad(format!("no runfiles beside {}: {error}", exe.display())))
}

/// Reads the value of one required environment entry.
fn env_value(name: &str) -> std::io::Result<String> {
    std::env::var(name)
        .map_err(|_| bad(format!("missing environment {name}")))
        .and_then(|value| {
            if value.is_empty() {
                return Err(bad(format!("missing environment {name}")));
            }
            Ok(value)
        })
}

/// Evaluates the manifest one runfiles key names for one executable.
pub fn run_with(exe: &Path, key: &str) -> std::io::Result<Report> {
    let resolver = resolver_for(exe)?;
    let manifest_path = resolver
        .lookup_from(key, "")
        .map_err(|error| bad(format!("missing runfile {key}: {error}")))?;
    let bytes = std::fs::read(&manifest_path)
        .map_err(|error| bad(format!("cannot read {}: {error}", manifest_path.display())))?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        bad(format!(
            "invalid manifest {}: {error}",
            manifest_path.display()
        ))
    })?;
    let mut report = evaluate(&manifest)?;
    evaluate_files(&manifest, &resolver, &mut report)?;
    evaluate_observations(&manifest, &mut report)?;
    summarize(&mut report);
    Ok(report)
}

/// Evaluates the manifest named by the test environment.
fn run() -> std::io::Result<Report> {
    let exe = match invoked_exe() {
        Ok(exe) => exe,
        Err(_) => {
            std::env::current_exe().map_err(|error| bad(format!("no test executable: {error}")))?
        }
    };
    let key = env_value(MANIFEST_ENV)?;
    run_with(&exe, &key)
}

fn bad(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.to_string())
}

/// Prints one evaluation result and returns the test exit code.
fn finish(
    result: &std::io::Result<Report>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> i32 {
    match result {
        Ok(report) => {
            if out
                .write_all(report.text().as_bytes())
                .and_then(|()| out.flush())
                .is_err()
            {
                return 1;
            }
            i32::from(!report.ok())
        }
        Err(error) => {
            let _ = writeln!(err, "starlark_harness: {error}");
            1
        }
    }
}

fn main() {
    std::process::exit(finish(
        &run(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn manifest(checks: serde_json::Value, files: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "version": 2,
            "name": "//pkg:probe",
            "checks": checks,
            "files": files,
            "observations": [],
        })
    }

    fn bare() -> serde_json::Value {
        manifest(serde_json::json!([]), serde_json::json!([]))
    }

    fn scratch() -> PathBuf {
        let base = std::env::var_os("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("starlark-harness-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn write(dir: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parents");
        }
        std::fs::write(&path, bytes).expect("write file");
        path
    }

    fn evaluated(value: serde_json::Value) -> Report {
        let mut report = evaluate(&value).expect("manifest evaluates");
        let dir = scratch();
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        evaluate_observations(&value, &mut report).expect("observations evaluate");
        summarize(&mut report);
        report
    }

    fn empty_report() -> Report {
        Report {
            lines: Vec::new(),
            passed: 0,
            failed: 0,
        }
    }

    fn files_on_tree(
        tree_files: &[(&str, &[u8])],
        manifest_value: &serde_json::Value,
    ) -> std::io::Result<Report> {
        let dir = scratch();
        let tree = dir.join("probe_test.runfiles");
        for (rel, bytes) in tree_files {
            write(&tree, rel, bytes);
        }
        let manifest_bytes = serde_json::to_string(manifest_value).expect("manifest json");
        write(
            &tree,
            "_main/pkg/case.starlark_manifest.json",
            manifest_bytes.as_bytes(),
        );
        let exe = write(&dir, "probe_test", b"stand-in");
        run_with(&exe, "_main/pkg/case.starlark_manifest.json")
    }

    #[test]
    fn equal_assertions_render_expected_and_actual() {
        let report = evaluated(manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
                {"kind": "equal", "name": "off", "expected": "3", "actual": "2"},
            ]),
            serde_json::json!([]),
        ));
        assert_eq!(
            report.lines,
            [
                "PASS: sum",
                "FAIL: off\n  expected: 3\n  actual:   2",
                "starlark_test: 1 passed, 1 failed",
            ]
        );
        assert!(!report.ok());
    }

    #[test]
    fn boolean_assertions_render_the_wanted_value() {
        let report = evaluated(manifest(
            serde_json::json!([
                {"kind": "true", "name": "yes", "actual": "True", "passed": "True"},
                {"kind": "true", "name": "no", "actual": "False", "passed": "False"},
                {"kind": "false", "name": "nay", "actual": "False", "passed": "True"},
                {"kind": "false", "name": "yea", "actual": "True", "passed": "False"},
            ]),
            serde_json::json!([]),
        ));
        assert_eq!(
            report.lines,
            [
                "PASS: yes",
                "FAIL: no\n  expected: True\n  actual:   False",
                "PASS: nay",
                "FAIL: yea\n  expected: False\n  actual:   True",
                "starlark_test: 2 passed, 2 failed",
            ]
        );
    }

    #[test]
    fn membership_and_substring_assertions_render_details() {
        let report = evaluated(manifest(
            serde_json::json!([
                {"kind": "contains", "name": "hit", "haystack": "abc", "needle": "b", "passed": "True"},
                {"kind": "contains", "name": "miss", "haystack": "abc", "needle": "z", "passed": "False"},
                {"kind": "match", "name": "found", "value": "[1, 2]", "want": "1", "passed": "True"},
                {"kind": "match", "name": "lost", "value": "[1, 2]", "want": "9", "passed": "False"},
            ]),
            serde_json::json!([]),
        ));
        assert_eq!(
            report.lines,
            [
                "PASS: hit",
                "FAIL: miss\n  haystack: abc\n  missing:  z",
                "PASS: found",
                "FAIL: lost\n  value: [1, 2]\n  missing substring: 9",
                "starlark_test: 2 passed, 2 failed",
            ]
        );
    }

    #[test]
    fn multiline_and_quoted_values_compare_exactly() {
        let report = evaluated(manifest(
            serde_json::json!([
                {"kind": "equal", "name": "lines", "expected": "a\nb\nc", "actual": "a\nb\nc"},
                {"kind": "equal", "name": "quotes", "expected": "say \"hi\" `now` $HOME \\ done", "actual": "say \"hi\" `now` $HOME \\ done"},
                {"kind": "equal", "name": "trailing", "expected": "a\n", "actual": "a"},
            ]),
            serde_json::json!([]),
        ));
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 1);
        assert_eq!(report.lines[0], "PASS: lines");
        assert_eq!(report.lines[1], "PASS: quotes");
        assert_eq!(
            report.lines[2],
            "FAIL: trailing\n  expected: a\n\n  actual:   a"
        );
    }

    #[test]
    fn an_unknown_check_kind_is_an_error() {
        let error = evaluate(&manifest(
            serde_json::json!([{"kind": "bogus", "name": "x"}]),
            serde_json::json!([]),
        ))
        .expect_err("unknown kind");
        assert!(error.to_string().contains("unknown kind"), "{error}");
    }

    #[test]
    fn a_missing_check_field_is_an_error() {
        let error = evaluate(&manifest(
            serde_json::json!([{"kind": "equal", "name": "x", "expected": "1"}]),
            serde_json::json!([]),
        ))
        .expect_err("missing field");
        assert!(error.to_string().contains("actual"), "{error}");
    }

    #[test]
    fn a_wrong_manifest_version_is_an_error() {
        let mut value = bare();
        value["version"] = serde_json::json!(3);
        let error = evaluate(&value).expect_err("version");
        assert!(error.to_string().contains("version 3"), "{error}");
    }

    #[test]
    fn a_version_one_manifest_is_rejected() {
        let mut value = bare();
        value["version"] = serde_json::json!(1);
        let error = evaluate(&value).expect_err("version");
        assert!(error.to_string().contains("version 1"), "{error}");
    }

    #[test]
    fn an_observations_only_manifest_prints_them_before_the_summary() {
        let mut value = bare();
        value["observations"] = serde_json::json!(["subject //pkg:a", "file a.txt"]);
        let report = evaluated(value);
        assert_eq!(
            report.lines,
            [
                "--- observations ---",
                "subject //pkg:a",
                "file a.txt",
                "starlark_test: 0 passed, 0 failed",
            ]
        );
        assert!(report.ok());
    }

    #[test]
    fn observations_follow_checks_and_files() {
        let mut value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
            ]),
            serde_json::json!([
                {"label": "//pkg:answer", "key": "_main/pkg/answer.txt", "needles": ["answer=42"]},
            ]),
        );
        value["observations"] = serde_json::json!(["subject //pkg:answer"]);
        let report = files_on_tree(&[("_main/pkg/answer.txt", b"answer=42\n")], &value)
            .expect("run with tree");
        assert_eq!(
            report.lines,
            [
                "PASS: sum",
                "PASS: file //pkg:answer contains substring 1/1",
                "--- observations ---",
                "subject //pkg:answer",
                "starlark_test: 2 passed, 0 failed",
            ]
        );
    }

    #[test]
    fn a_missing_observations_field_is_an_error() {
        let mut value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
            ]),
            serde_json::json!([]),
        );
        if let Some(object) = value.as_object_mut() {
            object.remove("observations");
        }
        let error = evaluate(&value).expect_err("observations");
        assert!(error.to_string().contains("observations"), "{error}");
    }

    #[test]
    fn a_non_array_observations_field_is_an_error() {
        let mut value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
            ]),
            serde_json::json!([]),
        );
        value["observations"] = serde_json::json!("subject //pkg:a");
        let error = evaluate(&value).expect_err("observations");
        assert!(error.to_string().contains("observations"), "{error}");
    }

    #[test]
    fn a_manifest_without_evidence_is_an_error() {
        let error = evaluate(&bare()).expect_err("evidence");
        assert!(error.to_string().contains("no evidence"), "{error}");
    }

    #[test]
    fn file_entries_check_every_needle_against_raw_bytes() {
        let dir = scratch();
        write(
            &dir,
            "_main/pkg/answer.txt",
            b"answer=42\nnote \"quoted\" $HOME `tap` \\ done\n",
        );
        write(&dir, "_main/pkg/shape.txt", b"shape=circle\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:answer", "key": "_main/pkg/answer.txt", "needles": ["answer=42", "note \"quoted\" $HOME `tap` \\ done"]},
                {"label": "//pkg:shape", "key": "_main/pkg/shape.txt", "needles": ["shape=circle"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        assert_eq!(
            report.lines,
            [
                "PASS: file //pkg:answer contains substring 1/2",
                "PASS: file //pkg:answer contains substring 2/2",
                "PASS: file //pkg:shape contains substring 1/1",
            ]
        );
        assert!(report.ok());
    }

    #[test]
    fn a_missing_substring_reports_needle_and_path() {
        let dir = scratch();
        write(&dir, "_main/pkg/present.txt", b"here\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:present", "key": "_main/pkg/present.txt", "needles": ["absent"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        assert_eq!(report.failed, 1);
        assert!(report.lines[0].contains("FAIL: file //pkg:present is missing substring 1/1"));
        assert!(report.lines[0].contains("substring: absent"));
        assert!(report.lines[0].contains("present.txt"));
    }

    #[test]
    fn a_multiline_needle_matches_only_contiguous_lines() {
        let dir = scratch();
        write(&dir, "_main/pkg/joined.txt", b"a\nb\n");
        write(&dir, "_main/pkg/split.txt", b"a\nx\nb\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:joined", "key": "_main/pkg/joined.txt", "needles": ["a\nb"]},
                {"label": "//pkg:split", "key": "_main/pkg/split.txt", "needles": ["a\nb"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        assert_eq!(report.failed, 1);
        assert_eq!(
            report.lines[0],
            "PASS: file //pkg:joined contains substring 1/1"
        );
        assert!(report.lines[1].contains("FAIL: file //pkg:split is missing substring 1/1"));
    }

    #[test]
    fn a_missing_multiline_needle_reports_only_the_missing_line() {
        let dir = scratch();
        write(&dir, "_main/pkg/block.txt", b"a\nb\nc\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:block", "key": "_main/pkg/block.txt", "needles": ["a\nb\nX\nc"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        assert_eq!(report.failed, 1);
        assert!(report.lines[0].contains("substring: X\n"));
        assert!(report.lines[0].contains("needle line: 3 of 4"));
        assert!(!report.lines[0].contains("a\nb"));
    }

    #[test]
    fn an_out_of_order_needle_line_is_reported() {
        let dir = scratch();
        write(&dir, "_main/pkg/split.txt", b"a\nx\nb\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:split", "key": "_main/pkg/split.txt", "needles": ["a\nb"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        evaluate_files(&value, &resolver, &mut report).expect("files evaluate");
        assert_eq!(report.failed, 1);
        assert!(report.lines[0].contains("substring: b\n"));
        assert!(report.lines[0].contains("needle line: 2 of 2"));
    }

    #[test]
    fn longest_prefix_match_handles_repeated_bytes() {
        assert_eq!(longest_prefix_match(b"aaxaab", b"aab"), 3);
        assert_eq!(longest_prefix_match(b"aaxa", b"aab"), 2);
        assert_eq!(longest_prefix_match(b"", b"aab"), 0);
    }

    #[test]
    fn a_missing_runfile_is_an_error() {
        let dir = scratch();
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:ghost", "key": "_main/pkg/ghost.txt", "needles": ["x"]},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        let error =
            evaluate_file(&value["files"][0], &resolver, &mut report).expect_err("missing runfile");
        assert!(error.to_string().contains("_main/pkg/ghost.txt"), "{error}");
    }

    #[test]
    fn a_file_entry_without_needles_is_an_error() {
        let dir = scratch();
        write(&dir, "_main/pkg/empty.txt", b"x\n");
        let value = manifest(
            serde_json::json!([]),
            serde_json::json!([
                {"label": "//pkg:empty", "key": "_main/pkg/empty.txt", "needles": []},
            ]),
        );
        let resolver = Resolver::for_tree(&dir).expect("tree resolver");
        let mut report = empty_report();
        let error = evaluate_files(&value, &resolver, &mut report).expect_err("needles");
        assert!(error.to_string().contains("no substrings"), "{error}");
    }

    #[test]
    fn run_with_resolves_manifest_and_files_through_argv_exe() {
        let value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
            ]),
            serde_json::json!([
                {"label": "//pkg:answer", "key": "_main/pkg/answer.txt", "needles": ["answer=42"]},
            ]),
        );
        let report = files_on_tree(&[("_main/pkg/answer.txt", b"answer=42\n")], &value)
            .expect("run with tree");
        assert_eq!(
            report.lines,
            [
                "PASS: sum",
                "PASS: file //pkg:answer contains substring 1/1",
                "starlark_test: 2 passed, 0 failed",
            ]
        );
        assert!(report.ok());
    }

    #[test]
    fn run_with_reports_failures_instead_of_passing() {
        let value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "off", "expected": "3", "actual": "2"},
            ]),
            serde_json::json!([
                {"label": "//pkg:answer", "key": "_main/pkg/answer.txt", "needles": ["absent"]},
            ]),
        );
        let report = files_on_tree(&[("_main/pkg/answer.txt", b"answer=42\n")], &value)
            .expect("run with tree");
        assert!(!report.ok());
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 2);
        assert_eq!(report.lines[2], "starlark_test: 0 passed, 2 failed");
    }

    fn finish_text(result: &std::io::Result<Report>) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = finish(result, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("utf-8 stdout"),
            String::from_utf8(err).expect("utf-8 stderr"),
        )
    }

    #[test]
    fn finish_exits_zero_when_every_check_passes() {
        let value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "sum", "expected": "3", "actual": "3"},
            ]),
            serde_json::json!([]),
        );
        let result = files_on_tree(&[], &value);
        let expected = result.as_ref().expect("run with tree").text();
        let (code, out, err) = finish_text(&result);
        assert_eq!(code, 0);
        assert_eq!(out, expected);
        assert_eq!(out, "PASS: sum\nstarlark_test: 1 passed, 0 failed\n");
        assert!(err.is_empty(), "{err}");
    }

    #[test]
    fn finish_exits_one_and_prints_fail_lines_when_a_check_fails() {
        let value = manifest(
            serde_json::json!([
                {"kind": "equal", "name": "off", "expected": "3", "actual": "2"},
            ]),
            serde_json::json!([
                {"label": "//pkg:answer", "key": "_main/pkg/answer.txt", "needles": ["absent"]},
            ]),
        );
        let result = files_on_tree(&[("_main/pkg/answer.txt", b"answer=42\n")], &value);
        let (code, out, err) = finish_text(&result);
        assert_eq!(code, 1);
        assert!(out.contains("FAIL: off"), "{out}");
        assert!(out.contains("starlark_test: 0 passed, 2 failed"), "{out}");
        assert!(err.is_empty(), "{err}");
    }

    #[test]
    fn finish_exits_one_on_an_evaluation_error() {
        let result = Err(bad("missing environment STARLARK_HARNESS_MANIFEST"));
        let (code, out, err) = finish_text(&result);
        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(
            err.starts_with("starlark_harness: missing environment"),
            "{err}"
        );
    }

    struct Broken;

    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn finish_exits_one_when_stdout_cannot_be_written() {
        let report = Report {
            lines: vec!["starlark_test: 0 passed, 0 failed".to_owned()],
            passed: 0,
            failed: 0,
        };
        let mut err = Vec::new();
        assert_eq!(finish(&Ok(report), &mut Broken, &mut err), 1);
    }

    #[test]
    fn run_with_rejects_an_unknown_manifest_key() {
        let dir = scratch();
        let exe = write(&dir, "probe_test", b"stand-in");
        std::fs::create_dir_all(dir.join("probe_test.runfiles/_main/pkg")).expect("empty tree");
        let error = run_with(&exe, "_main/pkg/absent.json").expect_err("unknown key");
        assert!(
            error.to_string().contains("_main/pkg/absent.json"),
            "{error}"
        );
    }

    #[test]
    fn a_tree_ancestor_selects_the_tree_resolver() {
        let root = tree_ancestor(Path::new("/x/y/test.runfiles/_main/pkg/test")).expect("tree");
        assert_eq!(root, PathBuf::from("/x/y/test.runfiles"));
        assert!(tree_ancestor(Path::new("/x/y/test")).is_none());
    }
}
