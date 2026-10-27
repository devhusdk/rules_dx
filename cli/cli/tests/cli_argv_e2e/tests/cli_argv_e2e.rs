//! `dx test --test_arg` reaches the test binary verbatim (#3774).
#![cfg(unix)]

use std::path::PathBuf;

use dx_testing::{mkscratch, Run};

mod support;

use support::{dx_in, fixture};

const UNICODE_ARG: &str = "--name=h\u{e9}llo w\u{f6}rld \u{65e5}\u{672c}";

fn repo(name: &str) -> PathBuf {
    let root = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    fixture(&root);
    root
}

fn argv_line(run: &Run) -> String {
    run.combined()
        .lines()
        .find_map(|line| {
            line.split_once("ARGV_JSON:")
                .map(|(_, rest)| rest.to_owned())
        })
        .unwrap_or_else(|| panic!("no argv in output: {}", run.combined()))
}

#[test]
fn test_unicode_arg_with_spaces_and_sentinel_arrive_exactly() {
    let root = repo("dx-argv-unicode");
    let unicode = format!("--test_arg={UNICODE_ARG}");
    let ran = dx_in(
        &root,
        &[
            "test",
            "//t:argv_test",
            "--",
            "--test_output=all",
            &unicode,
            "--test_arg=--sentinel",
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        argv_line(&ran),
        format!("[\"{UNICODE_ARG}\",\"--sentinel\"]"),
        "{}",
        ran.combined()
    );
}

#[test]
fn test_missing_sentinel_fails_the_test() {
    let root = repo("dx-argv-sentinel");
    let ran = dx_in(&root, &["test", "//t:argv_test", "--", "--test_output=all"]);
    assert_ne!(ran.status.code(), Some(0), "{}", ran.combined());
}

#[test]
fn test_ignored_flag_runs_ignored_case_and_default_does_not() {
    let root = repo("dx-argv-ignored");
    let with = dx_in(
        &root,
        &[
            "test",
            "//t:ignored_test",
            "--",
            "--test_output=all",
            "--test_arg=--ignored",
        ],
    );
    assert_eq!(with.status.code(), Some(0), "{}", with.combined());
    assert!(
        with.combined().contains("MARKER_WRITTEN"),
        "{}",
        with.combined()
    );
    assert_eq!(argv_line(&with), "[\"--ignored\"]", "{}", with.combined());

    let without = dx_in(
        &root,
        &["test", "//t:ignored_test", "--", "--test_output=all"],
    );
    assert_eq!(without.status.code(), Some(0), "{}", without.combined());
    assert!(
        !without.combined().contains("MARKER_WRITTEN"),
        "{}",
        without.combined()
    );
}
