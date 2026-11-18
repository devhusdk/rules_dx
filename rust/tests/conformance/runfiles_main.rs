#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! Prints a data file resolved through the runfiles library.

use std::process::ExitCode;

use runfiles::{Runfiles, rlocation};

fn main() -> ExitCode {
    match read_data() {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn read_data() -> Result<String, String> {
    let runfiles = Runfiles::create().map_err(|e| format!("runfiles: {e}"))?;
    let name = std::env::args().nth(1).ok_or("data path argument missing")?;
    let path = rlocation!(runfiles, &name).ok_or_else(|| format!("data file {name} must resolve"))?;
    std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}
