//! Forwards a repin request to dx update, which owns the pinned versions.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::Path;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let Some(workspace) =
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
    else {
        eprintln!("repin-all: BUILD_WORKSPACE_DIRECTORY is unset; run it with `bazel run //tools:repin-all`");
        return ExitCode::from(2);
    };
    dx_process::configure_real(Path::new(&workspace));
    dx_plan::plan::set_rules_dx_repo(Path::new(&workspace));
    // The shared settings keep this `run` on the same analysis cache as dx's own builds.
    let mut argv =
        match dx_plan::plan::bazel_argv("run", &[], &[], &[], &["//cli/cli:dx".to_owned()], &[]) {
            Ok(argv) => argv,
            Err(error) => {
                eprintln!("repin-all: cannot build the bazel command: {error}");
                return ExitCode::from(2);
            }
        };
    argv[0] = bazel();
    argv.extend(["--", "update"].map(str::to_owned));
    let status = Command::new(&argv[0])
        .current_dir(&workspace)
        .args(&argv[1..])
        .args(std::env::args_os().skip(1))
        .status();
    let code = match status {
        Ok(done) => done.code().unwrap_or(1),
        Err(error) => {
            eprintln!("repin-all: cannot run {}: {error}", bazel());
            1
        }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn bazel() -> String {
    std::env::var("DX_BAZEL_BIN").unwrap_or_else(|_| "bazel".to_string())
}
