//! The compiler of the nested `own_cc` toolchain: writes `own_cc` to every `-o` output and an empty `-MF` file, also inside `@` parameter files.

use std::fs;
use std::process::ExitCode;

fn expand(args: Vec<String>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for arg in args {
        match arg.strip_prefix('@') {
            Some(params) => {
                let text = fs::read_to_string(params).map_err(|error| format!("{params}: {error}"))?;
                out.extend(expand(
                    text.split_whitespace()
                        .map(|word| word.trim_matches(['\'', '"']).to_owned())
                        .collect(),
                )?);
            }
            None => out.push(arg),
        }
    }
    Ok(out)
}

fn run() -> Result<(), String> {
    let args = expand(std::env::args().skip(1).collect())?;
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let content = match word.as_str() {
            "-o" => "own_cc\n",
            "-MF" => "",
            _ => continue,
        };
        if let Some(path) = words.next() {
            fs::write(path, content).map_err(|error| format!("{path}: {error}"))?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("own_cc: {error}");
            ExitCode::FAILURE
        }
    }
}
