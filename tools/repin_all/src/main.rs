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
    let status = Command::new(bazel())
        .args(["run", "//cli/cli:dx", "--", "update", "--apply"])
        .args(std::env::args_os().skip(1))
        .status();
    let code = status.map_or(1, |done| done.code().unwrap_or(1));
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn bazel() -> String {
    std::env::var("DX_BAZEL_BIN").unwrap_or_else(|_| "bazel".to_string())
}
