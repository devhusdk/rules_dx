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
    if argv.len() >= 2 && argv[1] == "--members" {
        if argv.len() < 4 {
            return dx_deploy_tools::bin_usage("archiver", "--members <out> <name=path>...");
        }
        let dst = Path::new(&argv[2]);
        let mut members: Vec<(&str, &Path)> = Vec::new();
        for pair in &argv[3..] {
            match pair.split_once('=') {
                Some((name, path)) => members.push((name, Path::new(path))),
                None => {
                    eprintln!("archiver: member {pair:?} must be <name=path>");
                    return 1;
                }
            }
        }
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_members(&members, dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", dst, error),
        };
    }
    // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    if argv.len() != 3 {
        return dx_deploy_tools::bin_usage("archiver", "<src> <out>");
    }
    let src = Path::new(&argv[1]);
    let dst = Path::new(&argv[2]);
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match dx_deploy_tools::archive_file(src, dst) {
        Ok(()) => 0,
        Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", src, error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
