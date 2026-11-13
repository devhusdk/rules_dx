//! tools/ci/determinism_allowlist.txt is well formed and never grows past its cap.

use dx_testing::exec_log::parse_allowlist;

const ALLOWLIST: &str = "tools/ci/determinism_allowlist.txt";

const DETERMINISM_ALLOWLIST_CAP: usize = 5;

#[test]
fn the_allowlist_only_shrinks() {
    let text = dx_testing::read_runfiles(ALLOWLIST);
    let keys = parse_allowlist(&text).unwrap_or_else(|error| panic!("{ALLOWLIST}: {error}"));
    assert!(
        keys.len() <= DETERMINISM_ALLOWLIST_CAP,
        "{ALLOWLIST} has {} lines, the cap is {DETERMINISM_ALLOWLIST_CAP}; fix the action instead of listing it",
        keys.len()
    );
}
