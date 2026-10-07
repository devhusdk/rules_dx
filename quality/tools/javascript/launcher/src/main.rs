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
    let mut command = Command::new(&plan.program);
    command.args(&plan.node_args).arg("--").arg(&plan.entry);
    command.args(&plan.args);
    for (name, value) in &plan.env_set {
        command.env(name, value);
    }
    for (name, value) in &plan.env_default {
        if std::env::var_os(name).is_none() {
            command.env(name, value);
        }
    }
    prepend_path(&mut command, &plan.path_prefix);
    let status = command.status()?;
    Ok(std::process::ExitCode::from(
        status.code().unwrap_or(1) as u8
    ))
}

fn prepend_path(command: &mut Command, prefix: &std::path::Path) {
    if prefix.as_os_str().is_empty() {
        return;
    }
    let mut path = prefix.as_os_str().to_owned();
    if let Some(current) = std::env::var_os("PATH") {
        path.push(path_separator());
        path.push(current);
    }
    command.env("PATH", path);
}

fn path_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn bad(message: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.to_string())
}
