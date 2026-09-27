use super::super::test_support::*;
use crate::exec::{execute, Env};
use dx_digest::blake3 as digest;
use quality_result::proto;
use quality_result::proto::FileSnapshot;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

#[test]
fn check_mode_reports_findings_and_fails_on_changes() {
    let mut harness = Harness::new("check-fails");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
    assert_eq!(code, 1);
    assert!(out.contains("Running lint analysis for //..."));
    assert!(out.contains("warning src/a.py: unused [lint-tool/lint-tool/rule] (fixable)"));
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn check_mode_clean_run_succeeds() {
    let mut harness = Harness::new("check-clean");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for"));
    assert_eq!(err, "");
}

#[test]
fn check_mode_fails_on_replacement_without_diagnostics() {
    let mut harness = Harness::new("check-replacement-only");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
    assert_eq!(code, 1);
    assert!(out.contains("Running lint analysis for //..."));
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "check mode must launch Bazel exactly once"
    );
}

#[test]
fn default_mode_applies_and_hides_fixed_findings() {
    let mut harness = Harness::new("default-apply");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert!(out.contains("Applied 1 file(s)."));
    assert!(!out.contains("unused"));
}

#[test]
fn default_mode_applies_without_rerunning_bazel() {
    let mut harness = Harness::new("no-rerun-after-apply");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert!(out.contains("Applied 1 file(s)."));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "default apply must launch Bazel exactly once, no post-apply rerun"
    );
    let mut check = Harness::new("no-rerun-after-apply-check");
    check.write_source("src/a.py", "x = 1\n");
    check.results.insert(
        "//test:corpus".to_owned(),
        check.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![check.replacement(b"y")],
        ),
    );
    let (check_code, _, _) = check.run(&["lint", "--check", "--output=text"]);
    assert_eq!(check_code, 1);
    assert_eq!(
        std::fs::read(check.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert_eq!(
        check.seen_env.borrow().len(),
        1,
        "check mode must launch Bazel exactly once"
    );
}

#[test]
fn typecheck_and_format_apply_without_rerunning_bazel() {
    for (name, command) in [
        ("no-rerun-typecheck", "typecheck"),
        ("no-rerun-format", "format"),
    ] {
        let mut harness = Harness::new(name);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![Harness::diagnostic("unused", true)],
                vec![harness.replacement(b"y")],
            ),
        );
        let (code, out, _) = harness.run(&[command, "--output=text"]);
        assert_eq!(code, 0, "{command} default apply must succeed");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "{command} must apply the candidate"
        );
        assert!(out.contains("Applied 1 file(s)."), "{command} {out}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "{command} default apply must launch Bazel exactly once, no post-apply rerun"
        );
    }
}

#[test]
fn failed_target_prevents_mutation() {
    let harness = Harness {
        fail_target: true,
        ..Harness::new("partial")
    };
    harness.write_source("src/a.py", "x = 1\n");
    let (code, _, _) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn json_mode_emits_lifecycle_with_counts() {
    let mut harness = Harness::new("json");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1);
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "command_started",
            "diagnostic",
            "change",
            "command_finished"
        ]
    );
    let finished = events.last().expect("finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(true));
    assert_eq!(
        finished["diagnostics"],
        serde_json::json!({"info": 0, "warning": 1, "error": 0})
    );
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 1})
    );
}

#[test]
fn diff_mode_emits_patch_only() {
    let mut harness = Harness::new("diff");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(out.contains("--- a/src/a.py"));
    assert!(out.contains("+++ b/src/a.py"));
    assert!(!out.contains("Running lint"));
}

#[test]
fn sarif_file_report_writes_after_validation() {
    let mut harness = Harness::new("sarif");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    let (code, out, _) = harness.run(&[
        "lint",
        "--check",
        "--output=text",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(out.contains("Wrote sarif report to out.sarif."));
    let document = std::fs::read(harness.workspace.join("out.sarif")).expect("sarif");
    let parsed: serde_json::Value = serde_json::from_slice(&document).expect("JSON");
    assert_eq!(parsed["version"], serde_json::json!("2.1.0"));
    assert_eq!(
        parsed["runs"][0]["results"][0]["level"],
        serde_json::json!("warning")
    );
}

#[test]
fn dry_run_prints_summary_without_executing() {
    let harness = Harness::new("dry");
    let (code, out, _) = harness.run(&["lint", "--dry-run", "--", "--jobs=99"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //..."));
}

#[test]
fn conflicting_user_option_fails_before_execution() {
    let harness = Harness::new("conflict");
    let (code, _, err) = harness.run(&["lint", "--", "--nokeep_going"]);
    assert_eq!(code, 2);
    assert!(err.contains("usage"));
}

#[test]
fn file_scope_resolves_to_owners_before_planning() {
    let harness = Harness::new("file-scope");
    harness.write_source("pkg/BUILD.bazel", "");
    harness.write_source("pkg/a.py", "x = 1\n");
    harness.query.script_owners("//pkg:lib\n//pkg:extra\n");
    let (code, out, _) = harness.run(&["lint", "--dry-run", "pkg/a.py"]);
    assert_eq!(code, 0);
    assert!(
        out.contains("Running lint analysis for //pkg:extra //pkg:lib"),
        "{out}"
    );
    let calls = harness.query.calls.borrow();
    assert_eq!(calls.len(), 1, "one query per file");
    assert!(calls[0].iter().any(|arg| arg == "query"));
}

#[test]
fn directory_scope_plans_pattern_without_query() {
    let harness = Harness::new("dir-scope");
    harness.write_source("src/a.py", "x = 1\n");
    let (code, out, _) = harness.run(&["lint", "--dry-run", "src"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //src/..."), "{out}");
    assert!(harness.query.calls.borrow().is_empty());
}

#[test]
fn here_flag_selects_cwd_tree_without_query() {
    let mut harness = Harness::new("here-scope");
    harness.write_source("src/a.py", "x = 1\n");
    harness.cwd = harness.workspace.join("src");
    let (code, out, _) = harness.run(&["lint", "--dry-run", "--here"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //src/..."), "{out}");
    assert!(harness.query.calls.borrow().is_empty());

    let mut alias = Harness::new("here-alias");
    alias.write_source("src/a.py", "x = 1\n");
    alias.cwd = alias.workspace.join("src");
    let (code, out, _) = alias.run(&["lint", "--dry-run", "--cwd"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //src/..."), "{out}");

    let root = Harness::new("here-root");
    root.write_source("src/a.py", "x = 1\n");
    let (code, out, _) = root.run(&["lint", "--dry-run", "--here"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //..."), "{out}");
}

#[test]
fn missing_path_scope_fails_pre_execution() {
    let harness = Harness::new("missing-scope");
    let (code, _, err) = harness.run(&["lint", "nope.py"]);
    assert_eq!(code, 2);
    assert!(err.contains("nope.py"), "{err}");
    assert!(harness.query.calls.borrow().is_empty());
}

#[test]
fn external_scope_fails_pre_execution() {
    let harness = Harness::new("external-scope");
    let (code, _, err) = harness.run(&["lint", "@repo//pkg/..."]);
    assert_eq!(code, 2);
    assert!(err.contains("@repo//pkg/..."), "{err}");
}

#[test]
fn undecodable_artifact_marks_collection_incomplete() {
    let mut harness = Harness::new("undecodable");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        b"not-a-validated-result".to_vec(),
    );
    let (code, _, _) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn undecodable_sibling_blocks_valid_mutation() {
    let mut harness = Harness::new("partial-undecodable");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    harness.results.insert(
        "//other:corpus".to_owned(),
        b"not-a-validated-result".to_vec(),
    );
    let (code, out, _) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert!(!out.contains("Applied"));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "partial undecodable collection must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn launch_failure_is_operational() {
    let harness = Harness {
        io_error: true,
        ..Harness::new("launch-fail")
    };
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: launch_failed: failed to launch Bazel"));
}

#[test]
fn launch_failure_in_json_mode_emits_error_events() {
    let harness = Harness {
        io_error: true,
        ..Harness::new("launch-json")
    };
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 1);
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds, vec!["command_started", "error", "command_finished"]);
    let finished = events.last().expect("finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
}

#[test]
fn signalled_bazel_is_operational() {
    let harness = Harness {
        signalled: true,
        ..Harness::new("signalled")
    };
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: bazel_signalled: Bazel terminated by signal"));
}

#[test]
fn malformed_bep_stream_is_operational() {
    let harness = Harness {
        raw_bep: Some(vec!["{not json".to_owned()]),
        ..Harness::new("bad-bep")
    };
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: invalid_bep: invalid build events"));
}

#[test]
fn missing_bep_file_is_operational() {
    let harness = Harness {
        skip_bep: true,
        ..Harness::new("missing-bep")
    };
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: unreadable_bep: failed to read build events"));
}

#[test]
#[cfg(unix)]
fn non_utf8_temp_path_is_operational() {
    let harness = Harness::new("nonutf8-tmp");
    let mut raw = harness.temp.join("x").into_os_string().into_vec();
    raw.push(0xff);
    let temp = PathBuf::from(std::ffi::OsString::from_vec(raw));
    let invocation = invocation(&["lint", "--check"]);
    let runner = harness.runner();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &temp,
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    assert_eq!(code, 1);
    assert!(String::from_utf8(err)
        .expect("stderr")
        .contains("dx: unreadable_bep: temporary event path is not UTF-8"));
}

#[test]
fn dry_run_with_report_is_pre_exec() {
    let harness = Harness::new("dry-report");
    let (code, _, err) = harness.run(&["lint", "--dry-run", "--report=sarif=x.sarif"]);
    assert_eq!(code, 2);
    assert!(err.contains("usage: dx"));
}

#[test]
fn dry_run_json_emits_lifecycle() {
    let harness = Harness::new("dry-json");
    let (code, out, _) = harness.run(&["lint", "--dry-run", "--output=json"]);
    assert_eq!(code, 0);
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds, vec!["command_started", "command_finished"]);
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(0)
    );
}

#[test]
fn failed_bazel_with_changes_skips_mutation() {
    let mut harness = Harness {
        bazel_code: 1,
        ..Harness::new("bazel-fails")
    };
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert!(err.contains("Not applied: src/a.py (incomplete_collection)"));
}

#[test]
fn terminal_diagnostics_map_with_terminal_snapshot() {
    let mut harness = Harness::new("terminal-diag");
    harness.write_source("src/a.py", "x = 1\n");
    let digest = digest(b"x = 1\n").to_vec();
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.result_full(
            vec![],
            vec![Harness::diagnostic("terminal unused", false)],
            vec![],
            vec![FileSnapshot {
                path: "src/a.py".to_owned(),
                digest,
            }],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let diagnostics: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("diagnostic"))
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["snapshot"], serde_json::json!("terminal"));
}

#[test]
fn diff_mode_lists_unapplied_changes_after_failed_bazel() {
    let mut harness = Harness {
        bazel_code: 1,
        ..Harness::new("bazel-fails-diff")
    };
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, err) = harness.run(&["lint", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(out.contains("--- a/src/a.py"));
    assert!(err.contains("Not applied: src/a.py (incomplete_collection)"));
}

#[test]
fn json_mode_lists_unapplied_changes_after_failed_bazel() {
    let mut harness = Harness {
        bazel_code: 1,
        ..Harness::new("bazel-fails-json")
    };
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["mutations"],
        serde_json::json!({"applied": 0, "not_applied": 1})
    );
}

#[test]
fn stale_source_skips_mutation() {
    let mut harness = Harness::new("stale");
    harness.write_source("src/a.py", "x = 1\n");
    let bytes = harness.valid_result(
        vec![Harness::diagnostic("unused", true)],
        vec![harness.replacement(b"y")],
    );
    harness.results.insert("//test:corpus".to_owned(), bytes);
    harness.write_source("src/a.py", "z = 2\n");
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(err.contains("Not applied: src/a.py (stale_source)"));
}

#[test]
fn mixed_applied_and_not_applied_fail_together() {
    let mut harness = Harness::new("mixed-apply");
    harness.write_source("src/a.py", "x = 1\n");
    harness.write_source("src/b.py", "a = 1\n");
    let original_a = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let original_b = std::fs::read(harness.workspace.join("src/b.py")).expect("source");
    let change_a = harness.replacement_at(
        "src/a.py",
        digest(&original_a).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"y".to_vec(),
        }],
    );
    let change_b = harness.replacement_at(
        "src/b.py",
        digest(&original_b).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"b".to_vec(),
        }],
    );
    let bytes = harness.result_full(
        vec![
            Harness::diagnostic("unused", true),
            Harness::diagnostic_with(
                proto::Severity::Warning as i32,
                "lint-tool",
                "src/b.py",
                "unused",
                true,
            ),
        ],
        vec![],
        vec![change_a, change_b],
        vec![
            FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: digest(&original_a).to_vec(),
            },
            FileSnapshot {
                path: "src/b.py".to_owned(),
                digest: digest(&original_b).to_vec(),
            },
        ],
    );
    harness.results.insert("//test:corpus".to_owned(), bytes);
    harness.write_source("src/b.py", "z = 2\n");
    let (code, out, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(out.contains("Applied 1 file(s)."));
    assert!(err.contains("Not applied: src/b.py (stale_source)"));
}
