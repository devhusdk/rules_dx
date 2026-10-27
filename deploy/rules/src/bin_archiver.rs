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
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--members <out> [--executable <member>]... <name=path>...",
            );
        }
        let dst = Path::new(&argv[2]);
        let parsed = match dx_deploy_tools::member_args(&argv[3..]) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("archiver: {error}");
                return 1;
            }
        };
        let members: Vec<(&str, &Path, bool)> = parsed
            .iter()
            .map(|(name, path, executable)| (name.as_str(), path.as_path(), *executable))
            .collect();
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_members(&members, dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", dst, error),
        };
    }
    if argv.len() >= 2 && argv[1] == "--oci-layout" {
        // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        if argv.len() != 6 {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--oci-layout <out> <platform> <ref> <image>",
            );
        }
        let dst = Path::new(&argv[2]);
        let image = Path::new(&argv[5]);
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_oci_layout(image, &argv[3], &argv[4], dst) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot("archiver", "archive", image, error),
        };
    }
    if argv.len() >= 2 && argv[1] == "--image" {
        // LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        if argv.len() < 6 {
            return dx_deploy_tools::bin_usage(
                "archiver",
                "--image <out> <platform> <entrypoint> <layer>...",
            );
        }
        let dst = Path::new(&argv[2]);
        let entrypoint: Vec<&str> = argv[4].split_whitespace().collect();
        let layers: Vec<&Path> = argv[5..].iter().map(Path::new).collect();
        // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        return match dx_deploy_tools::archive_image(&layers, &argv[3], &entrypoint, dst) {
            Ok(_) => 0,
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
