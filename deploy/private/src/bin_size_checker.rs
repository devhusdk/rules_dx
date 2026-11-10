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
    let budget = argv.get(1).and_then(|text| text.parse::<u64>().ok());
    let (Some(budget), Some(stamp)) = (budget, argv.get(2)) else {
        return dx_deploy_tools::bin_usage("size_checker", "<budget_bytes> <stamp> <path>...");
    };
    let paths: Vec<PathBuf> = argv[3..].iter().map(PathBuf::from).collect();
    // LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    match dx_deploy_tools::check_files_size_budget(&paths, budget) {
        Ok(bytes) => match std::fs::write(stamp, format!("{bytes} of {budget} bytes\n")) {
            Ok(()) => 0,
            Err(error) => dx_deploy_tools::bin_cannot(
                "size_checker",
                "write",
                std::path::Path::new(stamp),
                error,
            ),
        },
        Err(message) => {
            eprintln!("size_checker: {message}");
            1
        }
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
