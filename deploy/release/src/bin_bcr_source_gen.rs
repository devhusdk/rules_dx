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
    if argv.len() != 4 {
        return dx_release_tools::bin_usage("bcr_source_gen", "<out> <module> <version>");
    }
    let dst = Path::new(&argv[1]);
    match dx_release_tools::write_bcr_source(dst, &argv[2], &argv[3]) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("bcr_source_gen", dst, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
