//! `gate_base_bin` pins origin/main for the affected checks and writes GATE_BASE to GITHUB_ENV.

use gate_base::{Event, export, pin};
use std::process::ExitCode;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn run() -> Result<String, String> {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY") {
        std::env::set_current_dir(&dir)
            .map_err(|error| format!("cannot enter {dir:?}: {error}"))?;
    }
    let event = Event {
        name: env("EVENT_NAME"),
        push_before: env("PUSH_BEFORE"),
        merge_group_base: env("MERGE_GROUP_BASE"),
    };
    let (base, head) = pin(&event)?;
    let file = std::env::var("GITHUB_ENV").map_err(|_| "GITHUB_ENV is not set".to_string())?;
    export(&file, &base)?;
    Ok(format!("gate: base {base} head {head}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
