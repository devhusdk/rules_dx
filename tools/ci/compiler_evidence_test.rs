use std::path::Path;
use std::process::Command;

use dx_test_scratch::{scratch, write};
use dx_testing::{read_runfiles, runfiles_root};
use serde_json::{Value, json};

const SCRIPT: &str = "tools/ci/compiler_evidence.sh";

fn producers_bin() -> Command {
    let bin = runfiles_root().join(std::env::var("DX_PRODUCERS_BIN").expect("DX_PRODUCERS_BIN"));
    let mut command = Command::new(bin);
    command.env_clear();
    command
}

fn evidence(root: &Path, command: &mut Command) -> std::process::Output {
    command
        .arg("--evidence")
        .arg(root.join("out.jsonl"))
        .arg("linux_x86_64")
        .arg(root.join("facts.json"))
        .env("DX_BIN", root.join("bin"))
        .env("DX_TESTLOGS", root.join("logs"))
        .env("DX_LOCKFILE", root.join("MODULE.bazel.lock"))
        .output()
        .expect("run producers_bin")
}

#[test]
fn evidence_mode_appends_a_line_per_compiler_row_from_its_environment() {
    let dir = scratch("compiler_evidence");
    let root = dir.path();
    let facts = json!({"rows": [
        {"name": "cc_a", "section": "compilers", "ci_tests": ["//p:a1"], "consumer_tests": [],
         "identity": {"compiler": "gcc", "version": "1.2.3", "probe": ""}},
        {"name": "other", "section": "languages", "ci_tests": ["//p:o"], "consumer_tests": [], "identity": {}},
    ]});
    write(root, "facts.json", &facts.to_string());
    write(
        root,
        "logs/p/a1/test.xml",
        "<testsuites><testcase/></testsuites>",
    );
    write(root, "bin/p/a1", "not a binary");
    write(root, "MODULE.bazel.lock", "lock\n");
    write(root, "out.jsonl", "{\"row\":\"kept\"}\n");
    let done = evidence(
        root,
        producers_bin()
            .env("DX_BAZEL_VERSION", "bazel 9\r\n")
            .env("DX_EVIDENCE_COMMAND", "bazel test //p:a1")
            .env("GITHUB_SHA", "abc")
            .env("GITHUB_SERVER_URL", "https://example.test")
            .env("GITHUB_REPOSITORY", "o/r")
            .env("GITHUB_RUN_ID", "7"),
    );
    assert!(done.status.success(), "{done:?}");
    let text = std::fs::read_to_string(root.join("out.jsonl")).expect("read evidence");
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("json line"))
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[0], json!({"row": "kept"}));
    assert_eq!(
        lines[1],
        json!({
            "platform": "linux_x86_64", "row": "cc_a", "compiler_flag": "gcc", "version": "1.2.3", "sha": "abc",
            "compiler": "", "target": "", "bazel": "bazel 9", "command": "bazel test //p:a1",
            "lockfile_sha256": "d8c9f2728aa278ebcd33ccedf3ad309a866870ad5fb93a03526b4b7655c9e911",
            "job": "https://example.test/o/r/actions/runs/7", "probe": "", "probe_outcome": "",
            "tests": {"//p:a1": "passed"}, "ran": 1, "total": 1, "observed_producers": [],
            "outcome": "passed", "tested_outcome": "passed", "blocker": "",
        })
    );
    assert!(
        text.lines().nth(1).is_some_and(
            |line| line.starts_with("{\"platform\":\"linux_x86_64\",\"row\":\"cc_a\",")
        ),
        "{text}"
    );
}

#[test]
fn evidence_mode_fails_without_the_bazel_version() {
    let dir = scratch("compiler_evidence");
    write(dir.path(), "facts.json", "{\"rows\": []}");
    let done = evidence(dir.path(), &mut producers_bin());
    assert!(!done.status.success(), "{done:?}");
    assert!(
        String::from_utf8_lossy(&done.stderr).contains("DX_BAZEL_VERSION is not set"),
        "{done:?}"
    );
}

#[test]
fn the_script_hands_its_arguments_to_producers_bin() {
    let script = read_runfiles(SCRIPT);
    let commands: Vec<&str> = script
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        commands,
        [
            "set -euo pipefail",
            "export DX_BAZEL_VERSION=\"${DX_BAZEL_VERSION:-$(bazel --version | tr -d '\\r')}\"",
            "exec \"$DX_PRODUCERS\" --evidence \"$@\"",
        ]
    );
}

#[test]
fn producers_bin_lists_nothing_for_a_non_elf_file() {
    let dir = scratch("compiler_evidence");
    write(dir.path(), "text", "plain");
    let output = producers_bin()
        .arg(dir.path().join("text"))
        .output()
        .expect("run producers_bin");
    let found: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(found[dir.path().join("text").to_str().unwrap()], json!([]));
}
