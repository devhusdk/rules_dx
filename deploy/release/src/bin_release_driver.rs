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

// LCOV_EXCL_START - reason: process spawn boundary, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
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
// LCOV_EXCL_STOP - reason: end process spawn boundary, issue: 1377, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: process entry, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    std::process::exit(dx_release_tools::driver_run(
        &std::env::args().collect::<Vec<_>>(),
        &|name| std::env::var(name).ok(),
        &git,
    ));
}
// LCOV_EXCL_STOP - reason: end process entry, issue: 1377, policy: docs/cli/commands/build-test-coverage.md
