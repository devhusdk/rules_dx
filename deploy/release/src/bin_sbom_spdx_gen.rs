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
    if argv.len() != 5 {
        return dx_release_tools::bin_usage(
            "sbom_spdx_gen",
            "<artifact> <out> <package> <supplier>",
        );
    }
    let src = Path::new(&argv[1]);
    let dst = Path::new(&argv[2]);
    match dx_release_tools::write_spdx(src, dst, &argv[3], &argv[4]) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("sbom_spdx_gen", dst, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
