mod check_continuation_common;

use check_continuation_common::*;

#[test]
fn a_clean_consumer_passes_every_phase() {
    let root = consumer(&CLEAN_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(0, 0, 0, 0), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 0);
    assert_eq!(finished(&all)["results_complete"], true);
    assert!(of_kind(&all, "diagnostic").is_empty(), "{}", run.stdout);
    assert_same(&root, &before);
}
