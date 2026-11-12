//! Every option in the root .bazelrc is named in tools/ci/root_bazelrc_allowlist.txt, and the list only shrinks.

use std::collections::BTreeSet;

use dx_testing::read_runfiles;

const ALLOWLIST: &str = "tools/ci/root_bazelrc_allowlist.txt";
const BAZELRC: &str = ".bazelrc";

const ROOT_BAZELRC_ALLOWLIST_CAP: usize = 48;

fn allowlist(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn option_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter(|word| word.starts_with("--"))
        .map(|word| word.split('=').next().unwrap_or(word).to_owned())
        .collect()
}

#[test]
fn every_root_option_is_listed_and_every_listed_name_is_used() {
    let listed = allowlist(&read_runfiles(ALLOWLIST));
    let used = option_names(&read_runfiles(BAZELRC));
    let unlisted: Vec<_> = used.difference(&listed).collect();
    assert!(
        unlisted.is_empty(),
        "{BAZELRC} sets options that dx does not generate and {ALLOWLIST} does not list: {unlisted:?}"
    );
    let unused: Vec<_> = listed.difference(&used).collect();
    assert!(unused.is_empty(), "{ALLOWLIST} lists names {BAZELRC} no longer sets: {unused:?}");
}

#[test]
fn the_allowlist_only_shrinks() {
    let listed = allowlist(&read_runfiles(ALLOWLIST));
    assert!(
        listed.len() <= ROOT_BAZELRC_ALLOWLIST_CAP,
        "{ALLOWLIST} has {} names, the cap is {ROOT_BAZELRC_ALLOWLIST_CAP}; move the flag to a lang.json setup.flags instead",
        listed.len()
    );
}

#[test]
fn option_names_skip_comments_and_imports() {
    let names = option_names("# build --x\ntry-import %workspace%/a\nbuild:linux --a=1\ncommon --b\n");
    assert_eq!(names, BTreeSet::from(["--a".to_owned(), "--b".to_owned()]));
}
