//! Forwards a repin request to dx update, which owns the pinned versions.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_default();
    let status = Command::new(bazel())
        .current_dir(dx_process::workspace_start(&cwd))
        .args(["run", "//cli/cli:dx", "--", "update"])
        .args(std::env::args_os().skip(1))
        .status();
    let code = status.map_or(1, |done| done.code().unwrap_or(1));
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn bazel() -> String {
    std::env::var("DX_BAZEL_BIN").unwrap_or_else(|_| "bazel".to_string())
}
