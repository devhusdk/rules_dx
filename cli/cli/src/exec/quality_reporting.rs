use super::super::test_support::*;
use dx_digest::blake3 as digest;
use quality_result::assert_all_equal;
use quality_result::proto;
use quality_result::proto::FileSnapshot;

#[test]
fn json_clean_check_succeeds() {
    let mut harness = Harness::new("json-clean");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 0);
    let events = json_events(&out);
    assert_eq!(
        event(&events, "command_finished")["exit_code"],
        serde_json::json!(0)
    );
}

#[test]
fn sarif_unreadable_snapshot_fails_report() {
    let mut harness = Harness::new("sarif-unreadable");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    std::fs::remove_file(harness.workspace.join("src/a.py")).expect("remove");
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        "--output=text",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: report_failed: failed to render SARIF report"));
}

#[test]
fn sarif_stale_snapshot_fails_report() {
    let mut harness = Harness::new("sarif-stale");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    harness.write_source("src/a.py", "changed = true\n");
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        "--output=text",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: report_failed: failed to render SARIF report"));
}

#[test]
fn sarif_non_utf8_snapshot_fails_report() {
    let mut harness = Harness::new("sarif-nonutf8");
    std::fs::create_dir_all(harness.workspace.join("src")).expect("dirs");
    std::fs::write(harness.workspace.join("src/a.py"), b"\xff").expect("bytes");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        "--output=text",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: report_failed: failed to render SARIF report"));
}

#[test]
fn sarif_stdout_report_owns_stdout() {
    let mut harness = Harness::new("sarif-stdout");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]),
    );
    let (code, out, err) = harness.run(&["lint", "--check", "--output=text", "--report=sarif=-"]);
    assert_eq!(code, 1);
    assert!(out.contains("2.1.0"));
    assert!(!out.contains("Running lint"));
    assert!(!err.contains("Running lint"));
    assert!(err.contains("unused"));
}

#[test]
fn json_report_write_failure_emits_error() {
    let mut harness = Harness::new("report-write-fail");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, out, err) = harness.run(&[
        "lint",
        "--check",
        "--output=json",
        "--report=sarif=nodir/out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: report_failed: failed to write sarif report to nodir/out.sarif"));
    let events = json_events(&out);
    assert_eq!(
        event(&events, "error")["code"],
        serde_json::json!("report_failed")
    );
}

#[test]
fn json_file_report_emits_report_event() {
    let mut harness = Harness::new("report-ok");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, out, _) = harness.run(&[
        "lint",
        "--check",
        "--output=json",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 0);
    let events = json_events(&out);
    let report = event(&events, "report");
    assert_eq!(report["format"], serde_json::json!("sarif"));
    assert!(harness.workspace.join("out.sarif").exists());
}

#[test]
fn diff_file_report_notes_to_stderr() {
    let mut harness = Harness::new("diff-report");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    let (code, _, err) = harness.run(&[
        "lint",
        "--check",
        "--output=diff",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 0);
    assert!(err.contains("Wrote sarif report to out.sarif."));
    assert!(harness.workspace.join("out.sarif").exists());
}

#[test]
fn unknown_tool_finding_fails_sarif_render() {
    let mut harness = Harness::new("unknown-tool");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic_with(
                proto::Severity::Warning as i32,
                "other-tool",
                "src/a.py",
                "stray",
                false,
            )],
            vec![],
        ),
    );
    let (code, out, err) = harness.run(&[
        "lint",
        "--check",
        "--output=json",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: report_failed: failed to render SARIF report"));
    let events = json_events(&out);
    assert_eq!(
        event(&events, "error")["code"],
        serde_json::json!("report_failed")
    );
}

#[test]
fn json_check_and_default_emit_identical_change_with_byte_equality() {
    fn changes(out: &str) -> Vec<serde_json::Value> {
        json_events(out)
            .into_iter()
            .filter(|event| event["event"] == serde_json::json!("change"))
            .collect()
    }
    let mut check = Harness::new("json-change-check");
    check.write_source("src/a.py", "x = 1\n");
    check.results.insert(
        "//test:corpus".to_owned(),
        check.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![check.replacement(b"y")],
        ),
    );
    let (check_code, check_out, _) = check.run(&["lint", "--check", "--output=json"]);
    assert_eq!(check_code, 1);
    assert_eq!(
        std::fs::read(check.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    let check_changes = changes(&check_out);
    assert_eq!(check_changes.len(), 1);
    let mut default = Harness::new("json-change-default");
    default.write_source("src/a.py", "x = 1\n");
    default.results.insert(
        "//test:corpus".to_owned(),
        default.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![default.replacement(b"y")],
        ),
    );
    let (default_code, default_out, _) = default.run(&["lint", "--output=json"]);
    assert_eq!(default_code, 0);
    assert_eq!(
        std::fs::read(default.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    let default_changes = changes(&default_out);
    assert_eq!(default_changes, check_changes);
    let change = &check_changes[0];
    assert_eq!(change["path"], serde_json::json!("src/a.py"));
    assert_eq!(change["kind"], serde_json::json!("modify"));
    let original = b"x = 1\n";
    let expected_digest = dx_digest::to_hex(&digest(original));
    assert_eq!(change["source_digest"], serde_json::json!(expected_digest));
    let edits = change["edits"].as_array().expect("edits");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["start_byte"], serde_json::json!(0));
    assert_eq!(edits[0]["end_byte"], serde_json::json!(1));
    assert_eq!(edits[0]["replacement"], serde_json::json!("y"));
    let mut planned = Vec::new();
    planned.extend_from_slice(&original[0..0]);
    planned.extend_from_slice(b"y");
    planned.extend_from_slice(&original[1..]);
    assert_eq!(planned, b"y = 1\n");
    assert_eq!(
        std::fs::read(default.workspace.join("src/a.py")).expect("source"),
        planned
    );
    let events = json_events(&default_out);
    let change_idx = events
        .iter()
        .position(|event| event["event"] == serde_json::json!("change"))
        .expect("change index");
    let mutation_idx = events
        .iter()
        .position(|event| event["event"] == serde_json::json!("mutation"))
        .expect("mutation index");
    assert!(
        change_idx < mutation_idx,
        "default mode must emit the change before its terminal mutation"
    );
}

#[test]
fn diff_check_and_default_emit_identical_patch_with_byte_equality() {
    let mut check = Harness::new("diff-patch-check");
    check.write_source("src/a.py", "x = 1\n");
    check.results.insert(
        "//test:corpus".to_owned(),
        check.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![check.replacement(b"y")],
        ),
    );
    let (check_code, check_out, _) = check.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(check_code, 1);
    assert_eq!(
        std::fs::read(check.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert_eq!(
        check.seen_env.borrow().len(),
        1,
        "diff check must launch Bazel exactly once, no rerun"
    );
    let mut default = Harness::new("diff-patch-default");
    default.write_source("src/a.py", "x = 1\n");
    default.results.insert(
        "//test:corpus".to_owned(),
        default.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![default.replacement(b"y")],
        ),
    );
    let (default_code, default_out, _) = default.run(&["lint", "--output=diff"]);
    assert_eq!(default_code, 0);
    assert_eq!(
        std::fs::read(default.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        default.seen_env.borrow().len(),
        1,
        "diff default must launch Bazel exactly once, no post-apply rerun"
    );
    assert_eq!(
        default_out, check_out,
        "diff check and default must emit byte-identical patches"
    );
    assert!(check_out.contains("--- a/src/a.py"));
    assert!(check_out.contains("+++ b/src/a.py"));
    assert!(check_out.contains("-x = 1"));
    assert!(check_out.contains("+y = 1"));
    let original = b"x = 1\n";
    let mut planned = Vec::new();
    planned.extend_from_slice(&original[0..0]);
    planned.extend_from_slice(b"y");
    planned.extend_from_slice(&original[1..]);
    assert_eq!(planned, b"y = 1\n");
    assert_eq!(
        std::fs::read(default.workspace.join("src/a.py")).expect("source"),
        planned
    );
}

#[test]
fn default_mode_applies_in_sorted_path_order_despite_reversed_arrival() {
    let mut harness = Harness::new("sorted-apply-order");
    harness.write_source("src/a.py", "x = 1\n");
    harness.write_source("src/b.py", "a = 1\n");
    harness.write_source("src/c.py", "m = 1\n");
    let original_a = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let original_b = std::fs::read(harness.workspace.join("src/b.py")).expect("source");
    let original_c = std::fs::read(harness.workspace.join("src/c.py")).expect("source");
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
    let change_c = harness.replacement_at(
        "src/c.py",
        digest(&original_c).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"z".to_vec(),
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
            Harness::diagnostic_with(
                proto::Severity::Warning as i32,
                "lint-tool",
                "src/c.py",
                "unused",
                true,
            ),
        ],
        vec![],
        vec![change_c, change_b, change_a],
        vec![
            FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: digest(&original_a).to_vec(),
            },
            FileSnapshot {
                path: "src/b.py".to_owned(),
                digest: digest(&original_b).to_vec(),
            },
            FileSnapshot {
                path: "src/c.py".to_owned(),
                digest: digest(&original_c).to_vec(),
            },
        ],
    );
    harness.results.insert("//test:corpus".to_owned(), bytes);
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"b = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/c.py")).expect("source"),
        b"z = 1\n"
    );
    let events = json_events(&out);
    let changes: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("change"))
        .collect();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0]["path"], serde_json::json!("src/a.py"));
    assert_eq!(changes[1]["path"], serde_json::json!("src/b.py"));
    assert_eq!(changes[2]["path"], serde_json::json!("src/c.py"));
    let mutations: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("mutation"))
        .collect();
    assert_eq!(mutations.len(), 3);
    assert_eq!(mutations[0]["path"], serde_json::json!("src/a.py"));
    assert_eq!(mutations[0]["outcome"], serde_json::json!("applied"));
    assert_eq!(mutations[1]["path"], serde_json::json!("src/b.py"));
    assert_eq!(mutations[1]["outcome"], serde_json::json!("applied"));
    assert_eq!(mutations[2]["path"], serde_json::json!("src/c.py"));
    assert_eq!(mutations[2]["outcome"], serde_json::json!("applied"));
    let terminals: std::collections::BTreeMap<&str, &[u8]> = [
        ("src/a.py", b"y = 1\n".as_slice()),
        ("src/b.py", b"b = 1\n".as_slice()),
        ("src/c.py", b"z = 1\n".as_slice()),
    ]
    .into_iter()
    .collect();
    let originals: std::collections::BTreeMap<&str, &[u8]> = [
        ("src/a.py", original_a.as_slice()),
        ("src/b.py", original_b.as_slice()),
        ("src/c.py", original_c.as_slice()),
    ]
    .into_iter()
    .collect();
    for prefix_len in 0..=3 {
        let mut expected: std::collections::BTreeMap<&str, &[u8]> = originals.clone();
        for mutation in mutations.iter().take(prefix_len) {
            let path = mutation["path"].as_str().expect("path");
            expected.insert(path, terminals[path]);
        }
        for (path, body) in &expected {
            let original = originals[path];
            let terminal = terminals[path];
            assert!(body == &original || body == &terminal);
        }
    }
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "sorted default apply must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn default_apply_depends_on_bytes_not_git_status() {
    let statuses = ["tracked", "modified", "staged", "untracked"];
    let mut digests = Vec::with_capacity(statuses.len());
    for status in statuses {
        let mut harness = Harness::new(&format!("git-status-{status}"));
        harness.write_source("src/a.py", "x = 1\n");
        harness.write_source(".git/HEAD", "ref: refs/heads/main\n");
        harness.write_source(&format!(".git/status-{status}"), status);
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(
                vec![Harness::diagnostic("unused", true)],
                vec![harness.replacement(b"y")],
            ),
        );
        let (code, out, _) = harness.run(&["lint", "--output=json"]);
        assert_eq!(code, 0, "{status}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "{status}"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "{status}: default apply must launch Bazel exactly once, no rerun"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join(".git/HEAD")).expect("git head"),
            b"ref: refs/heads/main\n",
            "{status}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join(format!(".git/status-{status}")))
                .expect("git status marker"),
            status.as_bytes(),
            "{status}"
        );
        let events = json_events(&out);
        let change = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("change"))
            .expect("change event");
        assert_eq!(change["path"], serde_json::json!("src/a.py"));
        digests.push(change["source_digest"].clone());
    }
    assert_all_equal(&digests);
    let mut stale = Harness::new("git-status-stale");
    stale.write_source("src/a.py", "x = 1\n");
    stale.write_source(".git/HEAD", "ref: refs/heads/main\n");
    stale.write_source(".git/status-staged", "staged");
    let bytes = stale.valid_result(
        vec![Harness::diagnostic("unused", true)],
        vec![stale.replacement(b"y")],
    );
    stale.results.insert("//test:corpus".to_owned(), bytes);
    stale.write_source("src/a.py", "z = 2\n");
    let (code, _, err) = stale.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("Not applied: src/a.py (stale_source)"));
    assert_eq!(
        std::fs::read(stale.workspace.join("src/a.py")).expect("source"),
        b"z = 2\n"
    );
}

#[test]
fn invalid_edits_in_one_file_do_not_block_valid_sibling() {
    let mut harness = Harness::new("invalid-edits-sibling");
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
            end_byte: 100,
            replacement: b"x".to_vec(),
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
    let (code, out, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        original_b
    );
    assert!(out.contains("Applied 1 file(s)."));
    assert!(err.contains("Not applied: src/b.py (invalid_edits)"));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "invalid sibling must launch Bazel exactly once, no rerun"
    );
}
