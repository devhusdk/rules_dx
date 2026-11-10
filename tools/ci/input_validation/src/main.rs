//! `input_validation_bin` validates the consumer workflow inputs and writes them to `GITHUB_OUTPUT`.
//! With the `deployables` argument it validates the release workflow list instead.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::Write;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    eprintln!("dx-ci: {message}");
    ExitCode::FAILURE
}

fn get(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// `input_validation_bin deployables` checks `DX_DEPLOYABLES` and prints one label per line.
fn deployables() -> ExitCode {
    match input_validation::validate_deployables(&get(input_validation::DEPLOYABLES_ENV)) {
        Ok(labels) => {
            for label in labels {
                println!("{label}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => fail(&message),
    }
}

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("deployables") {
        return deployables();
    }
    let validated = match input_validation::validate(get) {
        Ok(validated) => validated,
        Err(messages) => {
            for message in &messages {
                eprintln!("dx-ci: {message}");
            }
            return ExitCode::FAILURE;
        }
    };
    let path = get("GITHUB_OUTPUT");
    if path.is_empty() {
        return fail("GITHUB_OUTPUT is unset, so the validated inputs cannot reach the jobs.");
    }
    let text: String = validated
        .outputs
        .iter()
        .map(|(key, value)| {
            if value.contains('\n') {
                format!("{key}<<DX_EOF\n{value}\nDX_EOF\n")
            } else {
                format!("{key}={value}\n")
            }
        })
        .collect();
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if let Err(error) = written {
        return fail(&format!("cannot write {path}: {error}"));
    }
    for line in &validated.summary {
        println!("{line}");
    }
    ExitCode::SUCCESS
}
