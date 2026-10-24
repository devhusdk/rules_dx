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

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let Some(workspace) =
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
    else {
        eprintln!("repin-all: BUILD_WORKSPACE_DIRECTORY is unset; run it with `bazel run //tools:repin-all`");
        return ExitCode::from(2);
    };
    let status = Command::new(bazel())
        .current_dir(&workspace)
        .args(["run", "//cli/cli:dx", "--", "update"])
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
