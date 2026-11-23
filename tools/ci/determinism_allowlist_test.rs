use dx_testing::exec_log::parse_allowlist;

const ALLOWLIST: &str = "tools/ci/determinism_allowlist.txt";

#[test]
fn the_allowlist_stays_empty() {
    let text = dx_testing::read_runfiles(ALLOWLIST);
    let keys = parse_allowlist(&text).unwrap_or_else(|error| panic!("{ALLOWLIST}: {error}"));
    assert!(
        keys.is_empty(),
        "{ALLOWLIST} has {} lines; fix the action instead of listing it",
        keys.len()
    );
}
