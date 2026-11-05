use std::path::Path;
use std::process::Command;

use dx_test_scratch::{scratch, write};
use dx_testing::runfiles_root;
use serde_json::{json, Value};

const SCRIPT: &str = "tools/ci/compiler_evidence.sh";

fn facts() -> Value {
    json!({"rows": [
        {"name": "cc_a", "section": "compilers", "ci_tests": ["//p:a1", "//p:a2"], "consumer_tests": ["//q:a3"],
         "identity": {"compiler": "a", "version": "1.2.3", "probe": "//p:probe_a"}},
        {"name": "cc_b", "section": "compilers", "ci_tests": ["//p:b1"], "consumer_tests": [],
         "identity": {"compiler": "b", "version": "", "probe": ""}},
        {"name": "other", "section": "languages", "ci_tests": ["//p:o"], "consumer_tests": [], "identity": {}},
    ]})
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

fn run(root: &Path) -> Vec<Value> {
    write(root, "facts.json", &facts().to_string());
    let status = Command::new("bash")
        .arg(runfiles_root().join(SCRIPT))
        .arg(root.join("out.jsonl"))
        .arg("linux_x86_64")
        .arg(root.join("facts.json"))
        .env("DX_BIN", root.join("bin"))
        .env("DX_TESTLOGS", root.join("logs"))
        .env("DX_BAZEL_VERSION", "bazel 9")
        .status()
        .expect("run bash");
    assert!(status.success());
    std::fs::read_to_string(root.join("out.jsonl"))
        .expect("read evidence")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json line"))
        .collect()
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
    assert_eq!(lines.len(), 2);
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
