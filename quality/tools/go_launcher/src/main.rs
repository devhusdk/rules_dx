//! Launches a staged Go-toolchain tool with a hermetic SDK environment.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::process::Command;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> std::io::Result<std::process::ExitCode> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let plan = dx_go_launcher::plan(&args)?;
    let mut command = Command::new(&plan.tool);
    command.args(&plan.args);
    for (name, value) in &plan.env_set {
        command.env(name, value);
    }
    let status = command.status()?;
    Ok(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8
    ))
}
