mod check_continuation_common;

use check_continuation_common::*;

#[test]
fn a_later_clean_phase_never_clears_an_earlier_finding() {
    let root = consumer(&Sources {
        fmt: DIRTY_FORMAT,
        ..CLEAN_SOURCES
    });
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(1, 0, 0, 0), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 1);
    assert_eq!(finished(&all)["results_complete"], true);
    assert_same(&root, &before);
}
