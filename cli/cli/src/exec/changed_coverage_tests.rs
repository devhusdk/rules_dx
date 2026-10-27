use super::super::test_support::*;
use super::changed_lines;
use dx_plan::resolve::{QueryResult, QueryRunner};
use std::path::Path;

const SOURCES_ASPECT: &str = "//quality:coverage_sources.bzl%dx_coverage_sources_aspect";
const OWNS_A: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";
const HALF_LCOV: &str = "SF:src/a.py\nDA:1,1\nDA:2,0\nend_of_record\n";

fn gate_harness(name: &str, manifest: &str, lcov: &str) -> Harness {
    let harness = Harness::new(name);
    let lcov_uri = write_bep_artifact(&harness, "coverage.dat", lcov.as_bytes());
    let manifest_uri = write_bep_artifact(&harness, "sources.txt", manifest.as_bytes());
    let set = serde_json::json!({
        "id": {"namedSet": {"id": "s1"}},
        "namedSetOfFiles": {"files": [{"name": "lib.dx_coverage_sources.txt", "uri": manifest_uri}]},
    })
    .to_string();
    let completed = serde_json::json!({
        "id": {"targetCompleted": {"label": "//a:lib", "aspect": SOURCES_ASPECT}},
        "completed": {
            "success": true,
            "outputGroup": [{"name": "dx_coverage_sources", "fileSets": [{"id": "s1"}]}],
        },
    })
    .to_string();
    Harness {
        raw_bep: Some(vec![
            set,
            completed,
            test_result_line("//a:t", &[(String::from("test.lcov"), lcov_uri)]),
        ]),
        ..harness
    }
}

/// Scripts the default branch, its merge-base, the diff, and the untracked listing.
fn script_changes(harness: &Harness, diff: &str, untracked: &str) {
    harness.query.script_git(0, "origin/main\n");
    harness.query.script_git(0, "abc123\n");
    harness.query.script_git(0, diff);
    harness.query.script_git(0, untracked);
}

fn diff_adding(path: &str, start: u32, count: u32) -> String {
    let body: String = (0..count).map(|_| "+x\n").collect();
    format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -0,0 +{start},{count} @@\n{body}")
}

fn gate_event(out: &str) -> serde_json::Value {
    let events = json_events(out);
    let gates: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "changed_line_coverage")
        .collect();
    assert_eq!(gates.len(), 1, "{out}");
    gates[0].clone()
}

fn has_error(out: &str, code: &str) -> bool {
    json_events(out)
        .iter()
        .any(|event| event["event"] == "error" && event["code"] == code)
}

#[test]
fn covered_changed_lines_pass() {
    let harness = gate_harness("changed-pass", OWNS_A, HALF_LCOV);
    script_changes(&harness, &diff_adding("src/a.py", 1, 1), "");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains(
            "dx: changed-line coverage 100.00% (1/1 lines) meets minimum 80% against merge-base abc123"
        ),
        "{err}"
    );
    let calls = harness.query.git_calls.borrow();
    let diff = calls
        .iter()
        .find(|argv| argv.iter().any(|arg| arg == "-U0"))
        .expect("diff call");
    assert!(diff.iter().any(|arg| arg == "--relative"), "{diff:?}");
    assert_eq!(diff.last().map(String::as_str), Some("abc123"));
}

#[test]
fn covered_changed_lines_pass_in_json() {
    let harness = gate_harness("changed-pass-json", OWNS_A, HALF_LCOV);
    script_changes(&harness, &diff_adding("src/a.py", 1, 1), "");
    let (code, out, _) = harness.run(&[
        "coverage",
        "--output=json",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 0, "{out}");
    let event = gate_event(&out);
    assert_eq!(event["result"], "pass", "{event}");
    assert_eq!(event["base"], "origin/main", "{event}");
    assert_eq!(event["merge_base"], "abc123", "{event}");
    assert_eq!(event["minimum"], 80, "{event}");
    assert_eq!(event["covered_lines"], 1, "{event}");
    assert_eq!(event["eligible_lines"], 1, "{event}");
    assert_eq!(event["dx_version"], dx_adopt::DX_VERSION, "{event}");
}

#[test]
fn uncovered_changed_lines_fail_with_locations() {
    let harness = gate_harness("changed-fail", OWNS_A, HALF_LCOV);
    script_changes(&harness, &diff_adding("src/a.py", 1, 2), "");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: changed_line_coverage_below_minimum: changed-line coverage 50.00% (1/2 lines) below minimum 80% against merge-base abc123"),
        "{err}"
    );
    assert!(
        err.contains("dx: changed_line_uncovered: src/a.py:2"),
        "{err}"
    );
}

#[test]
fn uncovered_changed_lines_fail_in_json() {
    let harness = gate_harness("changed-fail-json", OWNS_A, HALF_LCOV);
    script_changes(&harness, &diff_adding("src/a.py", 1, 2), "");
    let (code, out, _) = harness.run(&[
        "coverage",
        "--output=json",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 1, "{out}");
    let event = gate_event(&out);
    assert_eq!(event["result"], "fail", "{event}");
    assert_eq!(
        event["uncovered"],
        serde_json::json!([{"path": "src/a.py", "line": 2}])
    );
    assert!(
        has_error(&out, "changed_line_coverage_below_minimum"),
        "{out}"
    );
}

#[test]
fn no_changed_executable_lines_is_not_applicable() {
    let harness = gate_harness("changed-none", OWNS_A, HALF_LCOV);
    script_changes(&harness, &diff_adding("README.md", 1, 3), "");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=100",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("dx: changed-line coverage not applicable: no changed executable lines against merge-base abc123"),
        "{err}"
    );
}

#[test]
fn no_changed_executable_lines_is_not_applicable_in_json() {
    let harness = gate_harness("changed-none-json", OWNS_A, HALF_LCOV);
    script_changes(&harness, "", "");
    let (code, out, _) = harness.run(&[
        "coverage",
        "--output=json",
        "--min-changed-line-coverage=100",
    ]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(gate_event(&out)["result"], "not-applicable");
    assert!(!out.contains("changed_line_coverage_"), "{out}");
}

#[test]
fn changed_file_missing_from_report_is_incomplete() {
    let harness = gate_harness(
        "changed-missing",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        HALF_LCOV,
    );
    harness.write_source("src/b.py", "b = 1\n");
    script_changes(&harness, &diff_adding("src/b.py", 1, 1), "");
    let (code, _, err) =
        harness.run(&["coverage", "--output=text", "--min-changed-line-coverage=0"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: changed_line_coverage_incomplete: changed sources absent from the coverage report against merge-base abc123: src/b.py"),
        "{err}"
    );
}

#[test]
fn changed_file_missing_from_report_is_incomplete_in_json() {
    let harness = gate_harness(
        "changed-missing-json",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n",
        HALF_LCOV,
    );
    harness.write_source("src/b.py", "b = 1\n");
    script_changes(&harness, "", "src/b.py\0.dx/state\0");
    let (code, out, _) =
        harness.run(&["coverage", "--output=json", "--min-changed-line-coverage=0"]);
    assert_eq!(code, 1, "{out}");
    let event = gate_event(&out);
    assert_eq!(event["result"], "incomplete", "{event}");
    assert_eq!(event["uninstrumented"], serde_json::json!(["src/b.py"]));
    assert!(has_error(&out, "changed_line_coverage_incomplete"), "{out}");
}

#[test]
fn untracked_source_counts_every_line() {
    let harness = gate_harness(
        "changed-untracked",
        "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/n.py\n",
        "SF:src/a.py\nDA:1,1\nend_of_record\nSF:src/n.py\nDA:1,1\nDA:2,0\nend_of_record\n",
    );
    harness.write_source("src/n.py", "a = 1\nb = 2\n");
    script_changes(&harness, "", "src/n.py\0");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=60",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("(1/2 lines) below minimum 60%"), "{err}");
    assert!(err.contains("changed_line_uncovered: src/n.py:2"), "{err}");
}

#[test]
fn missing_default_branch_is_unavailable() {
    let harness = gate_harness("changed-no-branch", OWNS_A, HALF_LCOV);
    let (code, out, _) = harness.run(&[
        "coverage",
        "--output=json",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(gate_event(&out)["result"], "unavailable");
    assert!(
        has_error(&out, "changed_line_coverage_unavailable"),
        "{out}"
    );
    assert!(out.contains("cannot find the default branch"), "{out}");
}

#[test]
fn shallow_clone_merge_base_failure_is_unavailable() {
    let harness = gate_harness("changed-shallow", OWNS_A, HALF_LCOV);
    harness.write_source(".git/shallow", "abc\n");
    harness.query.script_git(0, "origin/main\n");
    harness.query.script_git(128, "");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: changed_line_coverage_unavailable: git history is shallow"),
        "{err}"
    );
}

#[test]
fn diff_failure_is_unavailable() {
    let harness = gate_harness("changed-diff-fails", OWNS_A, HALF_LCOV);
    harness.query.script_git(0, "origin/main\n");
    harness.query.script_git(0, "abc123\n");
    harness.query.script_git(1, "");
    let (code, _, err) = harness.run(&[
        "coverage",
        "--output=text",
        "--min-changed-line-coverage=80",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("dx: changed_line_coverage_unavailable: git diff failed"),
        "{err}"
    );
}

#[test]
fn without_the_flag_dx_runs_no_git() {
    let harness = gate_harness("changed-absent", OWNS_A, HALF_LCOV);
    let (code, out, _) = harness.run(&["coverage", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    assert!(harness.query.git_calls.borrow().is_empty());
    assert!(!out.contains("changed_line_coverage"), "{out}");
}

/// Runs real `git` for `changed_lines`.
struct RealGit;

impl QueryRunner for RealGit {
    fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
        let output = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(cwd)
            .output()?;
        Ok(QueryResult {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

#[test]
fn real_git_counts_renames_untracked_files_and_a_nested_workspace() {
    let repo = temp_dir("changed-real-git");
    let root = repo.path();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=dx",
                "-c",
                "user.email=dx@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(root)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    };
    let write = |path: &str, text: &str| {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(full, text).expect("write");
    };
    let body: String = (1..=20).map(|n| format!("line_{n} = {n}\n")).collect();
    write("ws/src/old.py", &body);
    write("ws/src/keep.py", "a = 1\n");
    write("outside.py", "a = 1\n");
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "base"]);
    git(&["checkout", "--quiet", "-b", "topic"]);
    git(&["mv", "ws/src/old.py", "ws/src/new.py"]);
    write("ws/src/new.py", &body.replace("line_3 = 3", "line_3 = 33"));
    write("outside.py", "a = 2\n");
    git(&["commit", "--quiet", "-am", "rename"]);
    write("ws/src/my file.py", "x = 1\ny = 2\n");
    write("ws/.dx/state.txt", "ignored\n");
    let workspace = root.join("ws");
    let (branch, base) =
        super::named_base(&workspace, &RealGit).unwrap_or_else(|_| panic!("merge-base"));
    assert_eq!(branch, "main");
    let added = changed_lines(&workspace, &RealGit, &base).expect("changed lines");
    let keys: Vec<&str> = added.keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["src/my file.py", "src/new.py"], "{added:?}");
    assert_eq!(added["src/new.py"], [3].into_iter().collect());
    assert_eq!(added["src/my file.py"], [1, 2].into_iter().collect());
}
