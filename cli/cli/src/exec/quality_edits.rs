use super::super::test_support::*;
use dx_digest::blake3 as digest;
use quality_result::proto;
use quality_result::proto::FileSnapshot;

#[test]
fn json_mixed_applied_and_not_applied_fail_together() {
    let harness = two_file_replacements("json-mixed-apply");
    let original_a = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let original_b = std::fs::read(harness.workspace.join("src/b.py")).expect("source");
    harness.write_source("src/b.py", "z = 2\n");
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json", "--include-edits"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"z = 2\n"
    );
    let events = json_events(&out);
    let changes = events_of_kind(&events, "change");
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["path"], serde_json::json!("src/a.py"));
    assert_eq!(changes[1]["path"], serde_json::json!("src/b.py"));
    for (change, original) in [(&changes[0], &original_a), (&changes[1], &original_b)] {
        let expected_digest = dx_digest::to_hex(&digest(original));
        assert_eq!(change["source_digest"], serde_json::json!(expected_digest));
        let edits = change["edits"].as_array().expect("edits");
        assert_eq!(edits.len(), 1);
    }
    assert_eq!(
        changes[0]["edits"][0]["replacement"],
        serde_json::json!("y")
    );
    assert_eq!(
        changes[1]["edits"][0]["replacement"],
        serde_json::json!("b")
    );
    let mut planned_a = Vec::new();
    planned_a.extend_from_slice(&original_a[0..0]);
    planned_a.extend_from_slice(b"y");
    planned_a.extend_from_slice(&original_a[1..]);
    assert_eq!(planned_a, b"y = 1\n");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        planned_a
    );
    let mut intended_b = Vec::new();
    intended_b.extend_from_slice(&original_b[0..0]);
    intended_b.extend_from_slice(b"b");
    intended_b.extend_from_slice(&original_b[1..]);
    assert_eq!(intended_b, b"b = 1\n");
    assert_ne!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        intended_b
    );
    assert_eq!(changes[0]["effect"], serde_json::json!("applied"));
    assert!(changes[0].get("reason").is_none());
    assert_eq!(changes[1]["effect"], serde_json::json!("refused"));
    assert_eq!(changes[1]["reason"], serde_json::json!("stale_source"));
    assert!(events_of_kind(&events, "mutation").is_empty());
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 2, "applied": 1, "refused": 1})
    );
}

#[test]
fn json_changes_emit_in_sorted_path_order_despite_reversed_arrival() {
    let mut harness = two_file_replacements("json-sorted-order");
    let changes = vec![
        harness.replacement_in("src/b.py", b"b"),
        harness.replacement_in("src/a.py", b"y"),
    ];
    let bytes = harness.paths_result(&["src/a.py", "src/b.py"], changes);
    harness.results.insert("//test:corpus".to_owned(), bytes);
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"a = 1\n"
    );
    let events = json_events(&out);
    let changes = events_of_kind(&events, "change");
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["path"], serde_json::json!("src/a.py"));
    assert_eq!(changes[1]["path"], serde_json::json!("src/b.py"));
}

#[test]
fn missing_source_is_unreadable() {
    let mut harness = Harness::new("missing-src");
    harness.write_source("src/a.py", "x = 1\n");
    let change = harness.replacement_at(
        "src/missing.py",
        vec![0u8; 32],
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"y".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert!(err.contains("Not applied: src/missing.py (unreadable_source)"));
}

#[test]
fn out_of_bounds_edit_is_invalid() {
    let mut harness = Harness::new("oob-edit");
    harness.write_source("src/a.py", "x = 1\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 100,
            replacement: b"x".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert!(err.contains("Not applied: src/a.py (invalid_edits)"));
}

#[test]
fn multibyte_split_edit_is_invalid() {
    let mut harness = Harness::new("multibyte-split");
    harness.write_source("src/a.py", "héllo\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 1,
            end_byte: 2,
            replacement: b"X".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        original
    );
    assert!(err.contains("Not applied: src/a.py (invalid_edits)"));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "invalid multibyte split must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn non_utf8_source_is_invalid_in_apply_mode() {
    let mut harness = Harness::new("nonutf8-default");
    std::fs::create_dir_all(harness.workspace.join("src")).expect("dirs");
    std::fs::write(harness.workspace.join("src/a.py"), b"\xff\xfe").expect("bytes");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"y".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        original
    );
    assert!(err.contains("Not applied: src/a.py (invalid_edits)"));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "invalid non-UTF8 source must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn legacy_staging_dir_does_not_block_atomic_write() {
    let harness = umbrella_findings("staging-blocked");
    std::fs::create_dir_all(harness.workspace.join("src/.a.py.dx-apply-tmp")).expect("staging dir");
    let (code, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    assert!(out.contains("Applied 1 file(s)."));
    assert!(!err.contains("Not applied: src/a.py"));
}

#[test]
fn diff_missing_source_fails() {
    let mut harness = Harness::new("diff-missing");
    harness.write_source("src/a.py", "x = 1\n");
    let change = harness.replacement_at(
        "src/missing.py",
        vec![0u8; 32],
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"y".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot render patch without verified source for src/missing.py"));
}

#[test]
fn diff_non_utf8_source_fails() {
    let mut harness = Harness::new("diff-nonutf8");
    harness.write_source("src/a.py", "x = 1\n");
    std::fs::write(harness.workspace.join("src/a.py"), b"\xff\xfe").expect("bytes");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"y".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(err.contains("source for src/a.py is not UTF-8 text"));
}

#[test]
fn diff_unappliable_edit_fails() {
    let mut harness = Harness::new("diff-oob");
    harness.write_source("src/a.py", "x = 1\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 100,
            replacement: b"x".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot apply recorded edits for src/a.py"));
}

#[test]
fn diff_non_utf8_candidate_fails() {
    let mut harness = Harness::new("diff-candidate");
    harness.write_source("src/a.py", "héllo\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 1,
            end_byte: 2,
            replacement: b"X".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(err.contains("cannot apply recorded edits for src/a.py"));
}

#[test]
fn diff_identical_candidate_fails_render() {
    let mut harness = Harness::new("diff-noop");
    harness.write_source("src/a.py", "x = 1\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: b"x".to_vec(),
        }],
    );
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![change]),
    );
    let (code, _, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert!(err.contains("dx: diff_failed: failed to render patch"));
}

#[test]
fn diff_stale_source_fails_render() {
    let mut harness = Harness::new("diff-stale");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.write_source("src/a.py", "z = 2\n");
    let (code, out, err) = harness.run(&["lint", "--check", "--output=diff"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(err.contains("cannot render patch without verified source for src/a.py"));
    assert!(!out.contains("--- a/src/a.py"));
}

#[test]
fn diff_stale_source_fails_render_in_apply_mode() {
    let mut harness = Harness::new("diff-stale-default");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.write_source("src/a.py", "z = 2\n");
    let (code, out, err) = harness.run(&["lint", "--apply", "--output=diff"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(err.contains("cannot render patch without verified source for src/a.py"));
    assert!(!out.contains("--- a/src/a.py"));
    assert!(!out.contains("Applied"));
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "stale apply diff must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn json_apply_marks_remaining_resolution() {
    let mut harness = Harness::new("remaining");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", false)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    let events = json_events(&out);
    let remaining = event(&events, "diagnostic");
    assert_eq!(remaining["resolution"], serde_json::json!("remaining"));
    assert!(remaining.get("range").is_none(), "{out}");
    let change = event(&events, "change");
    assert_eq!(change["effect"], serde_json::json!("applied"));
    assert_eq!(change["edit_count"], serde_json::json!(1));
    assert!(
        change.get("edits").is_none(),
        "no replacement bodies: {out}"
    );
    assert!(!out.contains("replacement"), "{out}");
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 1, "applied": 1, "refused": 0})
    );
}

#[test]
fn json_apply_reports_a_reappearing_finding_once_at_its_written_position() {
    let mut harness = Harness::new("remaining-terminal");
    harness.write_source("src/a.py", "import os\nx = missing\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    let written = b"x = missing\n";
    let mut before = Harness::diagnostic("undefined", false);
    before.start_byte = Some(14);
    before.end_byte = Some(21);
    let mut after = before.clone();
    after.start_byte = Some(4);
    after.end_byte = Some(11);
    let change = harness.replacement_at(
        "src/a.py",
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 10,
            replacement: Vec::new(),
        }],
    );
    let bytes = harness.result_full(
        vec![before],
        vec![after],
        vec![change],
        vec![FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: digest(&original).to_vec(),
        }],
    );
    let mut result = quality_result::decode_validated(&bytes).expect("valid result");
    result.terminal_snapshot[0].digest = digest(written).to_vec();
    harness.results.insert(
        "//test:corpus".to_owned(),
        quality_result::encode_validated(&result).expect("encode"),
    );
    let (code, out, err) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        written
    );
    let events = json_events(&out);
    let diagnostics: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "diagnostic")
        .collect();
    assert_eq!(diagnostics.len(), 1, "{out}");
    let diagnostic = diagnostics[0];
    assert!(diagnostic.get("resolution").is_none(), "{diagnostic}");
    let range = &diagnostic["range"];
    assert_eq!(range["start_byte"], 4, "{diagnostic}");
    assert_eq!(range["end_byte"], 11, "{diagnostic}");
    assert_eq!(range["start_line"], 1, "{diagnostic}");
    assert_eq!(range["start_column"], 5, "{diagnostic}");
}

#[test]
fn json_apply_marks_not_applied_resolution() {
    let mut harness = Harness {
        bazel_code: 1,
        ..Harness::new("not-applied")
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
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    let events = json_events(&out);
    assert_eq!(
        event(&events, "diagnostic")["resolution"],
        serde_json::json!("not_applied")
    );
    let change = event(&events, "change");
    assert_eq!(change["effect"], serde_json::json!("refused"));
    assert_eq!(change["reason"], serde_json::json!("incomplete_collection"));
}

#[test]
fn json_apply_drops_applied_fixable_finding() {
    let harness = umbrella_findings("fixed-dropped");
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    let events = json_events(&out);
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == serde_json::json!("diagnostic")),
        "a finding the command fixed is never reported again: {out}"
    );
    assert_eq!(
        event(&events, "change")["effect"],
        serde_json::json!("applied")
    );
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["diagnostics"],
        serde_json::json!({"info": 0, "warning": 0, "error": 0})
    );
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 1, "applied": 1, "refused": 0})
    );
}

#[test]
fn json_counts_cover_all_severities() {
    let mut harness = Harness::new("counts");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![
                Harness::diagnostic_with(
                    proto::Severity::Info as i32,
                    "lint-tool",
                    "src/a.py",
                    "note",
                    false,
                ),
                Harness::diagnostic_with(
                    proto::Severity::Error as i32,
                    "lint-tool",
                    "src/a.py",
                    "broken",
                    false,
                ),
            ],
            vec![],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
    assert_eq!(code, 1);
    let events = json_events(&out);
    assert_eq!(
        event(&events, "command_finished")["diagnostics"],
        serde_json::json!({"info": 1, "warning": 0, "error": 1})
    );
}

fn write_failure(name: &str) -> (Harness, FailingWrites) {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    let fs = FailingWrites::new(&[harness.workspace.join("src/a.py")]);
    (harness, fs)
}

#[test]
fn diff_apply_write_failure_prints_the_patch_and_fails() {
    let (harness, fs) = write_failure("diff-write-failed");
    let (code, out, err) = harness.run_with_fs(&["format", "--apply", "--output=diff"], &fs);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("--- a/src/a.py"), "{out}");
    assert!(
        err.contains("Not applied: src/a.py (write_failed)"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn write_failure_fails_text_and_json_apply() {
    let (harness, fs) = write_failure("text-write-failed");
    let (code, out, err) = harness.run_with_fs(&["format", "--apply", "--output=text"], &fs);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("Not applied: src/a.py (write_failed)"),
        "{err}"
    );
    assert!(err.contains("next: "), "{err}");
    assert!(!out.contains("Applied"), "{out}");

    let (harness, fs) = write_failure("json-write-failed");
    let (code, out, err) = harness.run_with_fs(&["format", "--apply", "--output=json"], &fs);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let change = event(&events, "change");
    assert_eq!(change["effect"], serde_json::json!("refused"));
    assert_eq!(change["reason"], serde_json::json!("write_failed"));
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(
        finished["changes"],
        serde_json::json!({"create": 0, "modify": 1, "applied": 0, "refused": 1})
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
}

#[test]
fn write_failure_keeps_the_original_finding_in_sarif() {
    let harness = two_file_replacements("sarif-write-failed");
    let fs = FailingWrites::new(&[harness.workspace.join("src/a.py")]);
    let (code, out, err) = harness.run_with_fs(
        &[
            "lint",
            "--apply",
            "--output=text",
            "--report=sarif=out.sarif",
        ],
        &fs,
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("Not applied: src/a.py (write_failed)"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/b.py")).expect("source"),
        b"b = 1\n"
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("JSON");
    let results = document["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 1, "{document}");
    assert!(
        results[0].to_string().contains("src/a.py"),
        "the unwritten file keeps its original finding: {document}"
    );
}
