use super::super::test_support::*;
use dx_digest::blake3 as digest;
use quality_result::proto;
use quality_result::proto::FileSnapshot;

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
fn check_and_default_report_findings_without_writing() {
    for command in ["lint", "typecheck", "format"] {
        let default = umbrella_findings(&format!("default-check-{command}"));
        let (default_code, default_out, _) = default.run(&[command, "--output=text"]);
        let check = umbrella_findings(&format!("explicit-check-{command}"));
        let (check_code, check_out, _) = check.run(&[command, "--check", "--output=text"]);
        assert_eq!(default_code, 1, "{command}");
        assert_eq!(check_code, 1, "{command}");
        assert_eq!(default_out, check_out, "{command}");
        assert!(!default_out.contains("Applied"), "{command}");
        for harness in [&default, &check] {
            assert_eq!(
                std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
                b"x = 1\n",
                "{command} leaves bytes untouched"
            );
            assert_eq!(
                harness.seen_env.borrow().len(),
                1,
                "{command} must launch Bazel exactly once"
            );
        }
        if command == "lint" {
            assert!(check_out.contains("Running lint analysis for //..."));
            assert!(check_out.contains("src/a.py:1:1: warning: unused [lint-tool/rule] (fixable)"));
        }
    }
}

#[test]
fn apply_mode_applies_once_and_hides_fixed_findings() {
    for command in ["lint", "typecheck", "format"] {
        let harness = umbrella_findings(&format!("apply-once-{command}"));
        let (code, out, _) = harness.run(&[command, "--apply", "--output=text"]);
        assert_eq!(code, 0, "{command} apply must succeed");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "{command} must apply the candidate"
        );
        assert!(out.contains("Applied 1 file(s)."), "{command} {out}");
        assert!(!out.contains("unused"), "{command} {out}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "{command} apply must launch Bazel exactly once, no post-apply rerun"
        );
    }
}

#[test]
fn a_stream_without_bazels_last_event_never_reads_clean() {
    let mut harness = Harness::new("unterminated");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.unterminated_bep = true;
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let incomplete = events_of_kind(&events, "error")
        .into_iter()
        .find(|error| error["code"] == "incomplete_results")
        .expect("incomplete_results");
    assert_eq!(
        incomplete["message"],
        serde_json::json!("build events ended before Bazel's last event")
    );
    let finished = event(&events, "command_finished");
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("dx: incomplete_results: build events ended before Bazel's last event"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n",
        "missing evidence never authorizes an edit"
    );
}

#[test]
fn failed_target_prevents_mutation() {
    let harness = Harness {
        fail_target: true,
        ..Harness::new("partial")
    };
    harness.write_source("src/a.py", "x = 1\n");
    let (code, _, _) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn json_mode_emits_lifecycle_with_counts() {
    let harness = umbrella_findings("json");
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let kinds = event_kinds(&events);
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
    let harness = umbrella_findings("diff");
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
    harness.query.script_graph(&[
        graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
        graph_rule("//pkg:extra", &["//pkg:a.py"], &[]),
    ]);
    let (code, out, _) = harness.run(&["lint", "pkg/a.py"]);
    assert_eq!(code, 0);
    assert!(
        out.contains("Running lint analysis for //pkg:extra //pkg:lib"),
        "{out}"
    );
    let calls = harness.query.calls.borrow();
    assert_eq!(calls.len(), 1, "one rule graph query");
    assert_eq!(
        calls[0].last().map(String::as_str),
        Some("kind('rule', //...)")
    );
}

#[test]
fn directory_scope_plans_pattern_without_query() {
    let harness = Harness::new("dir-scope");
    harness.write_source("src/a.py", "x = 1\n");
    let (code, out, _) = harness.run(&["lint", "src"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //src/..."), "{out}");
    assert!(harness.query.calls.borrow().is_empty());
}

#[test]
fn here_flag_selects_cwd_tree_without_query() {
    let mut harness = Harness::new("here-scope");
    harness.write_source("src/a.py", "x = 1\n");
    harness.cwd = harness.workspace.join("src");
    let (code, out, _) = harness.run(&["lint", "--here"]);
    assert_eq!(code, 0);
    assert!(out.contains("Running lint analysis for //src/..."), "{out}");
    assert!(harness.query.calls.borrow().is_empty());

    let root = Harness::new("here-root");
    root.write_source("src/a.py", "x = 1\n");
    let (code, out, _) = root.run(&["lint", "--here"]);
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
    let (code, _, _) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn undecodable_sibling_blocks_valid_mutation() {
    let mut harness = umbrella_findings("partial-undecodable");
    harness.results.insert(
        "//other:corpus".to_owned(),
        b"not-a-validated-result".to_vec(),
    );
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=text"]);
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
    let events = json_events(&out);
    let kinds = event_kinds(&events);
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
    use crate::exec::execute;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    let harness = Harness::new("nonutf8-tmp");
    let mut raw = harness.temp.join("x").into_os_string().into_vec();
    raw.push(0xff);
    let temp = PathBuf::from(std::ffi::OsString::from_vec(raw));
    let invocation = invocation(&["lint", "--check"]);
    let runner = harness.runner();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut env = harness.env(&runner, &mut out, &mut err);
    env.temp_dir = &temp;
    let code = execute(&invocation, env);
    assert_eq!(code, 1);
    assert!(String::from_utf8(err)
        .expect("stderr")
        .contains("dx: unreadable_bep: temporary event path is not UTF-8"));
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
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
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
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let diagnostics = events_of_kind(&events, "diagnostic");
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
    let (code, out, err) = harness.run(&["lint", "--apply", "--output=diff"]);
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
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 1, "applied": 0, "refused": 1})
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
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
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
    let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
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

/// One stable result whose only content is a replacement of `src/a.py`.
fn replacement_only(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness
}

#[test]
fn stale_source_without_diagnostics_fails_every_quality_command() {
    for command in ["lint", "format", "typecheck"] {
        let harness = replacement_only(&format!("refused-text-{command}"));
        harness.write_source("src/a.py", "z = 2\n");
        let (code, out, err) = harness.run(&[command, "--apply", "--output=text"]);
        assert_eq!(code, 1, "{command}: {out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"z = 2\n",
            "{command}"
        );
        assert!(
            err.contains("Not applied: src/a.py (stale_source)"),
            "{command}: {err}"
        );
        assert!(err.contains("next: "), "{command}: {err}");

        let harness = replacement_only(&format!("refused-json-{command}"));
        harness.write_source("src/a.py", "z = 2\n");
        let (code, out, err) = harness.run(&[command, "--apply", "--output=json"]);
        assert_eq!(code, 1, "{command}: {out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"z = 2\n",
            "{command}"
        );
        let events = json_events(&out);
        let change = event(&events, "change");
        assert_eq!(change["effect"], serde_json::json!("refused"), "{command}");
        assert_eq!(
            change["reason"],
            serde_json::json!("stale_source"),
            "{command}"
        );
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1), "{command}");
        assert_eq!(
            finished["changes"],
            serde_json::json!({"create": 0, "modify": 1, "applied": 0, "refused": 1}),
            "{command}"
        );
    }
}

#[test]
fn mixed_apply_without_diagnostics_fails_and_keeps_the_written_file() {
    let harness = two_file_replacements("mixed-apply-no-diagnostics");
    let changes = vec![
        harness.replacement_in("src/a.py", b"y"),
        harness.replacement_in("src/b.py", b"b"),
    ];
    let original_a = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let original_b = std::fs::read(harness.workspace.join("src/b.py")).expect("source");
    let bytes = harness.result_full(
        vec![],
        vec![],
        changes,
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
    let mut harness = harness;
    harness.results.insert("//test:corpus".to_owned(), bytes);
    harness.write_source("src/b.py", "z = 2\n");
    let (code, out, err) = harness.run(&["format", "--apply", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(out.contains("Applied 1 file(s)."), "{out}");
    assert!(
        err.contains("Not applied: src/b.py (stale_source)"),
        "{err}"
    );
}

#[test]
fn clean_and_fully_applied_runs_still_succeed() {
    let mut clean = Harness::new("refused-control-clean");
    clean.write_source("src/a.py", "x = 1\n");
    clean.results.insert(
        "//test:corpus".to_owned(),
        clean.valid_result(vec![], vec![]),
    );
    let (code, out, err) = clean.run(&["format", "--apply", "--output=text"]);
    assert_eq!(code, 0, "{out}{err}");

    let applied = replacement_only("refused-control-applied");
    let (code, out, err) = applied.run(&["format", "--apply", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert_eq!(
        event(&events, "command_finished")["changes"],
        serde_json::json!({"create": 0, "modify": 1, "applied": 1, "refused": 0})
    );
}

#[test]
fn refusal_fails_even_when_findings_stay_below_the_threshold() {
    let mut harness = Harness::new("refused-below-threshold");
    harness.write_source("src/a.py", "x = 1\n");
    let info = Harness::diagnostic_with(
        proto::Severity::Info as i32,
        "lint-tool",
        "src/a.py",
        "note",
        false,
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![info.clone()], vec![]),
    );
    let (code, out, err) = harness.run(&["lint", "--apply", "--fail-on=error", "--output=text"]);
    assert_eq!(code, 0, "control: {out}{err}");

    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![info], vec![harness.replacement(b"y")]),
    );
    harness.write_source("src/a.py", "z = 2\n");
    let (code, out, err) = harness.run(&["lint", "--apply", "--fail-on=error", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("Not applied: src/a.py (stale_source)"),
        "{err}"
    );
}

#[test]
fn typed_quality_request_preserves_severity_to_the_exit_code() {
    for (args, code) in [
        (vec!["lint", "--fail-on=error", "--output=text"], 0),
        (vec!["lint", "--fail-on=warning", "--output=text"], 1),
    ] {
        let mut harness = Harness::new("typed-severity-");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
        );
        let (got, out, _) = harness.run(&args);
        assert_eq!(got, code, "{args:?}: {out}");
    }
}

#[test]
fn typed_quality_request_preserves_passthrough_and_default_scope() {
    let mut harness = Harness::new("typed-passthrough-");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, out, err) = harness.run(&["lint", "--output=text", "--", "--jobs=4"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running lint analysis for //..."), "{out}");
    assert_eq!(err, "", "{err}");
}

fn quality_phase(harness: &Harness, words: &[&str]) -> super::PhaseRun {
    let parsed = crate::args::parse(&crate::test_support::strings(words)).expect("parse");
    let inv = crate::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("here");
    let request = inv.quality_request().expect("quality request");
    let runner = harness.runner();
    let mut out = Vec::new();
    let mut err = Vec::new();
    super::execute_quality_phase(&request, harness.env(&runner, &mut out, &mut err))
}

#[test]
fn apply_phase_reports_written_effects_in_every_output_mode() {
    for output in ["--output=text", "--output=json", "--output=diff"] {
        let mut harness = Harness::new("phase-effects-written");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        let run = quality_phase(&harness, &["format", "--apply", output]);
        assert_eq!(run.code, 0, "{output}");
        assert_eq!(
            run.effects,
            super::PhaseEffects {
                changes: 1,
                applied: 1,
                not_applied: 0,
                baseline_refreshed: false,
            },
            "{output}"
        );
        assert!(run.effects.touched(), "{output}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "{output}"
        );
    }
}

#[test]
fn clean_apply_phase_touches_nothing() {
    let mut harness = Harness::new("phase-effects-clean");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let run = quality_phase(&harness, &["format", "--apply", "--output=text"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.kind, super::PhaseKind::Clean);
    assert_eq!(run.effects, super::PhaseEffects::default());
    assert!(!run.effects.touched());
}

#[test]
fn a_bazel_host_failure_ends_the_phase_as_host() {
    let mut disk_full = Harness::new("phase-kind-disk-full");
    disk_full.bazel_code = 36;
    disk_full.free_disk_bytes = Some(1 << 20);
    let mut launch_failed = Harness::new("phase-kind-launch-failed");
    launch_failed.io_error = true;
    let mut signalled = Harness::new("phase-kind-signalled");
    signalled.signalled = true;
    for (mut harness, code) in [(disk_full, 36), (launch_failed, 1), (signalled, 1)] {
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        let run = quality_phase(&harness, &["format", "--output=text"]);
        assert_eq!(run.code, code, "{:?}", run.kind);
        assert_eq!(run.kind, super::PhaseKind::Host, "exit {code}");
    }
}

#[test]
fn a_bazel_exit_with_partial_results_ends_the_phase_as_tool() {
    let mut harness = Harness::new("phase-kind-tool");
    harness.write_source("src/a.py", "x = 1\n");
    harness.bazel_code = 1;
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let run = quality_phase(&harness, &["format", "--output=text"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.kind, super::PhaseKind::Tool);
}

#[test]
fn a_compile_only_bazel_exit_ends_the_phase_as_compile() {
    let harness = Harness::new("phase-kind-compile");
    let uri = stderr_file(&harness, "rustc", RENDERED_E0308);
    let action = failed_action("//app:bin", "c1", "Rustc", "out/bin", &uri);
    let harness = Harness {
        bazel_code: 1,
        raw_bep: Some(vec![action]),
        ..harness
    };
    let run = quality_phase(&harness, &["lint", "--output=text"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.kind, super::PhaseKind::Compile);
    assert!(!run.effects.touched());
}

#[test]
fn a_refused_change_ends_the_phase_as_findings() {
    let harness = replacement_only("phase-kind-refused");
    harness.write_source("src/a.py", "z = 2\n");
    let run = quality_phase(&harness, &["format", "--apply", "--output=text"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.kind, super::PhaseKind::Findings);
    assert_eq!(
        run.effects,
        super::PhaseEffects {
            changes: 1,
            applied: 0,
            not_applied: 1,
            baseline_refreshed: false,
        }
    );
}
