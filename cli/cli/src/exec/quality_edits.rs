use super::super::test_support::*;
use dx_digest::blake3 as digest;
use quality_result::proto;
use quality_result::proto::FileSnapshot;

#[test]
fn json_mixed_applied_and_not_applied_fail_together() {
    let mut harness = Harness::new("json-mixed-apply");
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
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
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
    let changes: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("change"))
        .collect();
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
    let mutations: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("mutation"))
        .collect();
    assert_eq!(mutations.len(), 2);
    assert_eq!(mutations[0]["path"], serde_json::json!("src/a.py"));
    assert_eq!(mutations[0]["outcome"], serde_json::json!("applied"));
    assert_eq!(mutations[1]["path"], serde_json::json!("src/b.py"));
    assert_eq!(mutations[1]["outcome"], serde_json::json!("not_applied"));
    assert_eq!(mutations[1]["reason"], serde_json::json!("stale_source"));
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
    let finished = event(&events, "command_finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(
        finished["mutations"],
        serde_json::json!({"applied": 1, "not_applied": 1})
    );
}

#[test]
fn json_changes_emit_in_sorted_path_order_despite_reversed_arrival() {
    let mut harness = Harness::new("json-sorted-order");
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
        vec![change_b, change_a],
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
    let changes: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == serde_json::json!("change"))
        .collect();
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
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
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
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
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
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
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
fn non_utf8_source_is_invalid_in_default_mode() {
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
    let (code, _, err) = harness.run(&["lint", "--output=text"]);
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
    let mut harness = Harness::new("staging-blocked");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    std::fs::create_dir_all(harness.workspace.join("src/.a.py.dx-apply-tmp")).expect("staging dir");
    let (code, out, err) = harness.run(&["lint", "--output=text"]);
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
fn diff_stale_source_fails_render_in_default_mode() {
    let mut harness = Harness::new("diff-stale-default");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.write_source("src/a.py", "z = 2\n");
    let (code, out, err) = harness.run(&["lint", "--output=diff"]);
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
        "stale default diff must launch Bazel exactly once, no rerun"
    );
}

#[test]
fn json_default_marks_remaining_resolution() {
    let mut harness = Harness::new("remaining");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", false)],
            vec![harness.replacement(b"y")],
        ),
    );
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 1);
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n"
    );
    let events = json_events(&out);
    assert_eq!(
        event(&events, "diagnostic")["resolution"],
        serde_json::json!("remaining")
    );
    assert_eq!(
        event(&events, "mutation")["outcome"],
        serde_json::json!("applied")
    );
    let finished = event(&events, "command_finished");
    assert_eq!(
        finished["mutations"],
        serde_json::json!({"applied": 1, "not_applied": 0})
    );
}

#[test]
fn json_default_marks_not_applied_resolution() {
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
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
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
    let mutation = event(&events, "mutation");
    assert_eq!(mutation["outcome"], serde_json::json!("not_applied"));
    assert_eq!(
        mutation["reason"],
        serde_json::json!("incomplete_collection")
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
