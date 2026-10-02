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
use std::path::PathBuf;

fn run(argv: &[String]) -> i32 {
    if argv.len() < 5 {
        return dx_release_tools::bin_usage(
            "notice_gen",
            "<manifest> <out> <root> <text> [<text> ...]",
        );
    }
    let manifest = PathBuf::from(&argv[1]);
    let dst = PathBuf::from(&argv[2]);
    let texts: Vec<PathBuf> = argv[4..].iter().map(PathBuf::from).collect();
    match dx_release_tools::write_notice(&manifest, &dst, &argv[3], &texts) {
        Ok(()) => 0,
        Err(error) => dx_release_tools::bin_cannot_write("notice_gen", &dst, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
