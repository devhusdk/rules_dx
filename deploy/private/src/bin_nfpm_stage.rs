#![deny(clippy::indexing_slicing, clippy::string_slice)]

// LCOV_EXCL_START - reason: thin wrapper, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

const NFPM_TIMEOUT: Duration = Duration::from_mins(15);

fn forward(out: &dx_process::capture::Captured) -> io::Result<()> {
    io::stdout().write_all(&out.stdout)?;
    io::stderr().write_all(&out.stderr)
}

fn run(argv: &[String]) -> i32 {
    let usage = || dx_deploy_tools::bin_usage("nfpm_stage", "<stage> <src>... -- <nfpm> <arg>...");
    let Some(split) = argv.iter().position(|a| a == "--") else {
        return usage();
    };
    let (head, tail) = argv.split_at(split);
    let (Some((stage, srcs)), Some((nfpm, args))) = (
        head.get(1..).and_then(|h| h.split_first()),
        tail.get(1..).and_then(|t| t.split_first()),
    ) else {
        return usage();
    };
    let stage = Path::new(stage);
    // LCOV_EXCL_STOP - reason: end thin wrapper, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    if let Err(error) = dx_deploy_tools::stage_regular_files(stage, srcs) {
        return dx_deploy_tools::bin_cannot("nfpm_stage", "stage files into", stage, error);
    }
    match dx_process::capture::run_captured(
        std::process::Command::new(nfpm).args(args),
        NFPM_TIMEOUT,
    ) {
        Ok(out) => match forward(&out) {
            Ok(()) => out.code.unwrap_or(1),
            Err(error) => dx_deploy_tools::bin_cannot(
                "nfpm_stage",
                "forward the output of",
                Path::new(nfpm),
                error,
            ),
        },
        Err(error) => dx_deploy_tools::bin_cannot("nfpm_stage", "run", Path::new(nfpm), error),
    }
}

fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}
