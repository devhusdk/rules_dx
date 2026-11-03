//! `evidence_inputs_bin` validates the evidence dispatch inputs and writes them to `GITHUB_OUTPUT`.

use std::io::Write;
use std::process::ExitCode;

fn get(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn main() -> ExitCode {
    let validated = match evidence_inputs::validate(get) {
        Ok(validated) => validated,
        Err(message) => {
            println!("evidence input rejected: {message}");
            return ExitCode::FAILURE;
        }
    };
    let text: String = validated
        .outputs
        .iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect();
    let path = get("GITHUB_OUTPUT");
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if let Err(error) = written {
        eprintln!("cannot write GITHUB_OUTPUT {path:?}: {error}");
        return ExitCode::FAILURE;
    }
    println!("{}", validated.summary);
    ExitCode::SUCCESS
}
