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
    let manifest = dx_testing::read_json(&PathBuf::from(value(args, "--manifest")?))?;
    let label = manifest["label"]
        .as_str()
        .ok_or_else(|| "manifest has no label".to_owned())?;
    let languages: Vec<String> = manifest["languages"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let parsed = dx_testing::read_json(&evidence)?;
    hermetic::verify_evidence(&parsed, &commit, label, &languages)
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
