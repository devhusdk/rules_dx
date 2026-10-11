#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::Command;

use dx_release_tools::{
    release_driver_run, release_dry_requested, release_tag, release_tag_exists, release_tree_dirty,
};

// LCOV_EXCL_START - reason: process spawn, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                None
            }
        })
}
// LCOV_EXCL_STOP - reason: end process spawn, issue: 1377, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: env wiring, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
fn run(argv: &[String]) -> i32 {
    let tag = release_tag(argv).to_owned();
    let approve = std::env::var("RELEASE_APPROVE").unwrap_or_else(|_| "0".to_owned());
    let dry = release_dry_requested(std::env::var("RELEASE_DRY_RUN").ok());
    release_driver_run(
        argv,
        &approve,
        dry,
        release_tree_dirty(git(&["status", "--porcelain"])),
        release_tag_exists(git(&["tag", "--list", "v*"]), &tag),
    )
}
// LCOV_EXCL_STOP - reason: end env wiring, issue: 1377, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: process entry, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end process entry, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
