use super::{read_skip_notes, skip_notice};

fn target(label: &str, set: &str) -> [String; 1] {
    [serde_json::json!({
        "id": {"targetCompleted": {"label": label, "aspect": "//q:a.bzl%real_lint_aspect"}},
        "completed": {"success": true, "outputGroup": [{"name": "dx_skips", "fileSets": [{"id": set}]}]},
    })
    .to_string()]
}

fn named_set(dir: &std::path::Path, set: &str, text: &str) -> String {
    let file = dir.join(format!("{set}.txt"));
    std::fs::write(&file, text).expect("note file");
    serde_json::json!({
        "id": {"namedSet": {"id": set}},
        "namedSetOfFiles": {"files": [{"uri": format!("file://{}", file.display())}]},
    })
    .to_string()
}

#[test]
fn a_tool_dropped_by_two_targets_is_noted_once() {
    let dir = tempfile::tempdir().expect("temp");
    let note = "hlint: skipped, no pinned artifact for this execution platform (unsupported on macos_arm64)\n";
    let mut lines = vec![
        named_set(dir.path(), "1", note),
        named_set(dir.path(), "2", note),
    ];
    lines.extend(target("//a:a", "1"));
    lines.extend(target("//b:b", "2"));
    let bep = dir.path().join("events.json");
    std::fs::write(&bep, lines.join("\n")).expect("bep");
    assert_eq!(
        read_skip_notes(&bep, dir.path()),
        vec![note.trim_end().to_owned()]
    );
}

#[test]
fn a_skip_note_becomes_a_tool_skipped_notice() {
    let note = "hlint: skipped, no pinned artifact for this execution platform";
    let event = dx_output::notice_event(&skip_notice("lint", note)).expect("valid notice");
    assert_eq!(event["code"], "tool_skipped");
    assert_eq!(event["level"], "warning");
    assert_eq!(event["related_command"], "lint");
    assert_eq!(event["message"], note);
}
