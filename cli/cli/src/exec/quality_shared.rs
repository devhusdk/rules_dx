use super::super::test_support::*;
use dx_digest::blake3 as digest;
use quality_result::proto;
use quality_result::proto::FileSnapshot;

fn result_for(
    harness: &Harness,
    original: &[u8],
    original_digest: &[u8],
    terminal: &[u8],
    edits: Vec<proto::Edit>,
) -> Vec<u8> {
    let _ = original;
    harness.result_full(
        vec![Harness::diagnostic("unused", true)],
        vec![],
        vec![harness.replacement_at("src/a.py", original_digest.to_vec(), edits)],
        vec![FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: digest(terminal).to_vec(),
        }],
    )
}

fn edit(replacement: &[u8]) -> Vec<proto::Edit> {
    vec![proto::Edit {
        start_byte: 0,
        end_byte: 1,
        replacement: replacement.to_vec(),
    }]
}

fn setup(name: &str) -> (Harness, Vec<u8>) {
    let harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    let original = std::fs::read(harness.workspace.join("src/a.py")).expect("source");
    (harness, original)
}

fn source(harness: &Harness) -> Vec<u8> {
    std::fs::read(harness.workspace.join("src/a.py")).expect("source")
}

#[test]
fn disagreeing_candidates_leave_the_file_untouched_in_every_order() {
    for swap in [false, true] {
        let (mut harness, original) = setup("shared-conflict");
        let d = digest(&original).to_vec();
        let one = result_for(&harness, &original, &d, b"y = 1\n", edit(b"y"));
        let two = result_for(&harness, &original, &d, b"z = 1\n", edit(b"z"));
        let (first, second) = if swap { (two, one) } else { (one, two) };
        harness.results.insert("//test:one".to_owned(), first);
        harness.results.insert("//test:two".to_owned(), second);
        let (code, out, err) = harness.run(&["lint", "--apply", "--output=json"]);
        assert_eq!(code, 1);
        assert_eq!(source(&harness), original);
        let events = json_events(&out);
        let refusals = events_of_kind(&events, "change");
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0]["path"], serde_json::json!("src/a.py"));
        assert_eq!(refusals[0]["effect"], serde_json::json!("refused"));
        assert_eq!(refusals[0]["edit_count"], serde_json::json!(0));
        assert_eq!(
            refusals[0]["reason"],
            serde_json::json!("conflicting_digests")
        );
        assert_eq!(
            event(&events, "command_finished")["changes"],
            serde_json::json!({"create": 0, "modify": 0, "applied": 0, "refused": 1})
        );
        assert!(err.contains("Conflict: src/a.py"), "{err}");
        assert!(
            err.contains("//test:one") && err.contains("//test:two"),
            "{err}"
        );
    }
}

#[test]
fn identical_candidates_apply_once() {
    let (mut harness, original) = setup("shared-identical");
    let d = digest(&original).to_vec();
    let terminal = b"y = 1\n";
    let one = result_for(&harness, &original, &d, terminal, edit(b"y"));
    let two = result_for(&harness, &original, &d, terminal, edit(b"y"));
    harness.results.insert("//test:one".to_owned(), one);
    harness.results.insert("//test:two".to_owned(), two);
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 0);
    assert_eq!(source(&harness), terminal);
    let events = json_events(&out);
    let changes = events_of_kind(&events, "change");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["effect"], serde_json::json!("applied"));
    assert!(
        events_of_kind(&events, "diagnostic").is_empty(),
        "a shared finding both producers fixed is never reported again: {out}"
    );
    let finished = events_of_kind(&events, "command_finished")[0];
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
fn different_edit_encodings_of_the_same_bytes_agree() {
    let (mut harness, original) = setup("shared-encodings");
    let d = digest(&original).to_vec();
    let terminal = b"y = 1\n";
    let one = result_for(&harness, &original, &d, terminal, edit(b"y"));
    let two = result_for(
        &harness,
        &original,
        &d,
        terminal,
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 5,
            replacement: b"y = 1".to_vec(),
        }],
    );
    harness.results.insert("//test:one".to_owned(), one);
    harness.results.insert("//test:two".to_owned(), two);
    let (code, _, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 0);
    assert_eq!(source(&harness), terminal);
}

#[test]
fn differing_original_digests_conflict_in_check_mode() {
    let (mut harness, original) = setup("shared-original");
    let d = digest(&original).to_vec();
    let one = result_for(&harness, &original, &d, b"y = 1\n", edit(b"y"));
    let two = result_for(
        &harness,
        &original,
        &digest(b"other\n").to_vec(),
        b"y = 1\n",
        edit(b"y"),
    );
    harness.results.insert("//test:one".to_owned(), one);
    harness.results.insert("//test:two".to_owned(), two);
    let (code, _, err) = harness.run(&["lint"]);
    assert_eq!(code, 1);
    assert_eq!(source(&harness), original);
    assert!(
        err.contains("Conflict: src/a.py (conflicting_digests"),
        "{err}"
    );
    assert!(err.contains("next: resolve the Conflict files"), "{err}");
}

#[test]
fn text_mode_reports_one_not_applied_file_for_a_conflict() {
    let (mut harness, original) = setup("shared-text");
    let d = digest(&original).to_vec();
    let one = result_for(&harness, &original, &d, b"y = 1\n", edit(b"y"));
    let two = result_for(&harness, &original, &d, b"y = 1\n", edit(b"z"));
    harness.results.insert("//test:one".to_owned(), one);
    harness.results.insert("//test:two".to_owned(), two);
    let (code, _, err) = harness.run(&["lint", "--apply"]);
    assert_eq!(code, 1);
    assert_eq!(source(&harness), original);
    assert!(err.contains("conflicting_candidates"), "{err}");
}

#[test]
fn json_check_mode_proposes_apply_in_a_notice_but_apply_does_not() {
    let (mut harness, original) = setup("shared-proposed");
    let d = digest(&original).to_vec();
    let one = result_for(&harness, &original, &d, b"y = 1\n", edit(b"y"));
    harness.results.insert("//test:one".to_owned(), one);
    let (code, out, _) = harness.run(&["lint", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let notice = event(&events, "notice");
    assert_eq!(notice["code"], serde_json::json!("changes_proposed"));
    assert_eq!(notice["related_command"], serde_json::json!("lint"));
    assert!(
        notice["message"]
            .as_str()
            .is_some_and(|m| m.contains("--apply")),
        "{notice}"
    );
    let (code, out, _) = harness.run(&["lint", "--apply", "--output=json"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        events_of_kind(&json_events(&out), "notice").is_empty(),
        "{out}"
    );
}
