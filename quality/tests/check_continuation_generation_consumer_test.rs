mod check_continuation_common;

use check_continuation_common::*;

#[test]
fn generation_drift_alone_fails_before_three_clean_phases() {
    let root = consumer(&Sources {
        drift: true,
        ..CLEAN_SOURCES
    });
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(0, 0, 0, 1), "{}", run.stdout);
    assert_eq!(of_kind(&all, "diagnostic").len(), 0, "{}", run.stdout);
    assert_eq!(of_kind(&all, "change").len(), 1, "{}", run.stdout);
    assert_same(&root, &before);
}
