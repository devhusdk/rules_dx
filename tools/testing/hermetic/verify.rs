#![deny(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::unreachable,
    clippy::todo
)]

use std::path::PathBuf;
use std::process::ExitCode;

fn value(args: &[String], flag: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|arg| arg == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn run(args: &[String]) -> Result<(), String> {
    let evidence = PathBuf::from(value(args, "--evidence")?);
    let commit = value(args, "--commit")?;
    let label = value(args, "--label")?;
    let languages: Vec<String> = value(args, "--languages")?
        .split(',')
        .map(str::to_owned)
        .collect();
    let parsed = dx_testing::read_json(&evidence)?;
    hermetic::verify_evidence(&parsed, &commit, &label, &languages)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => {
            println!("hermetic evidence holds");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("hermetic evidence rejected: {err}");
            ExitCode::FAILURE
        }
    }
}
