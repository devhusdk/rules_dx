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
use std::path::Path;

fn run(argv: &[String]) -> i32 {
    if argv.len() != 3 {
        return dx_deploy_tools::bin_usage("hasher", "<src> <out>");
    }
    let src = Path::new(&argv[1]);
    let dst = Path::new(&argv[2]);
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match dx_deploy_tools::hash_file(src, dst) {
        Ok(()) => 0,
        Err(error) => dx_deploy_tools::bin_cannot("hasher", "hash", src, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
