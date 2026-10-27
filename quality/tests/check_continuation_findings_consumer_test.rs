mod check_continuation_common;

use check_continuation_common::*;

#[test]
fn one_check_returns_every_independent_finding_and_leaves_bytes_unchanged() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(1, 1, 1, 1), "{}", run.stdout);
    assert_eq!(of_kind(&all, "phase_started").len(), 4, "{}", run.stdout);
    let last = finished(&all);
    assert_eq!(last["event"], "command_finished", "{}", run.stdout);
    assert_eq!(last["exit_code"], 1);
    assert_eq!(last["results_complete"], true, "{}", run.stdout);
    let rules: Vec<(String, String, String)> = of_kind(&all, "diagnostic")
        .iter()
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["tool"].as_str().unwrap_or("?").to_owned(),
                event["rule"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        rules,
        vec![
            (
                "check/format".to_owned(),
                "ruff".to_owned(),
                "unformatted".to_owned()
            ),
            (
                "check/lint".to_owned(),
                "ruff".to_owned(),
                "F401".to_owned()
            ),
            (
                "check/typecheck".to_owned(),
                "ty".to_owned(),
                "invalid-return-type".to_owned()
            ),
        ],
        "{}",
        run.stdout
    );
    let changes: Vec<(String, String)> = of_kind(&all, "change")
        .iter()
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["path"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    assert!(
        changes.contains(&("check/generate".to_owned(), "gen/BUILD.bazel".to_owned())),
        "{changes:?}"
    );
    assert!(
        of_kind(&all, "change")
            .iter()
            .all(|event| event["effect"] == "proposed"),
        "{}",
        run.stdout
    );
    let noticed: Vec<String> = of_kind(&all, "notice")
        .iter()
        .filter(|event| event["code"] == "umbrella_phase_findings")
        .map(|event| event["correlation"].as_str().unwrap_or("?").to_owned())
        .collect();
    assert_eq!(
        noticed,
        [
            "check/generate",
            "check/format",
            "check/lint",
            "check/typecheck"
        ],
        "{}",
        run.stdout
    );
    assert!(
        of_kind(&all, "notice")
            .iter()
            .all(|event| event["code"] != "umbrella_phase_skipped"),
        "{}",
        run.stdout
    );
    assert_same(&root, &before);
}
