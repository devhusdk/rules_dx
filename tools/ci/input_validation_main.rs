//! `input_validation_bin` validates the consumer workflow inputs and writes them to `GITHUB_OUTPUT`.

use std::io::Write;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    eprintln!("dx-ci: {message}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    fn get(name: &str) -> String {
        std::env::var(name).unwrap_or_default()
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
        .map(|(key, value)| format!("{key}={value}\n"))
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
