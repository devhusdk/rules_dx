#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
use std::process::Command;

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

fn tree_dirty() -> bool {
    git(&["status", "--porcelain"]).is_some_and(|out| !out.trim().is_empty())
}

fn tag_exists(tag: &str) -> bool {
    git(&["tag", "--list", "v*"]).is_some_and(|out| out.lines().any(|line| line.trim() == tag))
}

fn run(argv: &[String]) -> i32 {
    let tag = argv.get(1).map(String::as_str).unwrap_or("v0.0.0-dryrun");
    let approve = std::env::var("RELEASE_APPROVE").unwrap_or_else(|_| "0".to_owned());
    let dry = std::env::var("RELEASE_DRY_RUN").unwrap_or_else(|_| "1".to_owned()) != "0";
    match dx_release_tools::release_run(tag, &approve, dry, tree_dirty(), tag_exists(tag)) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => dx_release_tools::bin_error(diagnostic),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
