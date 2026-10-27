mod check_continuation_common;

use check_continuation_common::*;

#[test]
fn a_rejected_scope_stops_before_any_later_phase() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "no/such/dir", &["--output=json"]);
    assert_eq!(run.status.code(), Some(2), "{}", run.combined());
    let all = events(&run);
    assert_eq!(of_kind(&all, "phase_started").len(), 1, "{}", run.stdout);
    assert_eq!(
        phase_exits(&all),
        vec![("generate".to_owned(), 2)],
        "{}",
        run.stdout
    );
    let errors = of_kind(&all, "error");
    assert_eq!(errors.len(), 1, "{}", run.stdout);
    assert_eq!(errors[0]["code"], "invalid_scope", "{}", run.stdout);
    let skipped: Vec<(String, String)> = of_kind(&all, "notice")
        .iter()
        .filter(|event| event["code"] == "umbrella_phase_skipped")
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["message"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    let expected: Vec<(String, String)> = ["format", "lint", "typecheck"]
        .iter()
        .map(|phase| {
            (
                format!("check/{phase}"),
                format!("{phase} skipped after generate rejected its scope or configuration"),
            )
        })
        .collect();
    assert_eq!(skipped, expected, "{}", run.stdout);
    assert!(of_kind(&all, "diagnostic").is_empty(), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 2, "{}", run.stdout);
    assert_eq!(finished(&all)["results_complete"], false, "{}", run.stdout);
    assert_same(&root, &before);
}
