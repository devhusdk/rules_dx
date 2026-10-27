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
    let manifest_path = PathBuf::from(value(args, "--manifest")?);
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("cannot read {}: {err}", manifest_path.display()))?;
    let base = manifest_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default();
    let manifest = hermetic::parse_manifest(&text, &base)?;
    let parsed = dx_testing::read_json(&evidence)?;
    hermetic::verify_evidence(&parsed, &commit, &manifest)
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
