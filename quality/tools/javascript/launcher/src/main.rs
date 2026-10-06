//! Runs a generated JavaScript tool through the node runtime beside it.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;
use std::process::Command;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("js_launcher: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> std::io::Result<std::process::ExitCode> {
    let argv: Vec<String> = std::env::args().collect();
    let exe = argv
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| bad("no argv[0]"))?;
    let plan = dx_js_launcher::plan(&exe, argv.get(1..).unwrap_or_default())?;
    let status = Command::new(&plan.program)
        .arg(&plan.entry)
        .args(&plan.args)
        .status()?;
    Ok(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8
    ))
}

fn bad(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.to_string())
}
