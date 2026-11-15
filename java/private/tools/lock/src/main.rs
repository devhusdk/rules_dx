//! The lock step of the maven ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::PathBuf;
use std::process::ExitCode;

use maven_lock::{Outcome, Request, Tools, lock};

fn usage() -> ExitCode {
    eprintln!("usage: lock --dir <set directory> [--check]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut dir = None;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--check" => check = true,
            _ => return usage(),
        }
    }
    let Some(dir) = dir else {
        return usage();
    };
    let Some(hub) = dx_families::hub("maven") else {
        eprintln!("maven has no registry entry");
        return ExitCode::from(2);
    };
    let tools = match Tools::from_env() {
        Ok(tools) => tools,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let request = Request {
        dir,
        manifest: hub.manifest.clone(),
        lock: hub.lock.clone().unwrap_or_default(),
        repositories: Vec::new(),
        check,
    };
    match lock(&tools, &request) {
        Ok(Outcome::Fresh | Outcome::Written) => ExitCode::SUCCESS,
        Ok(Outcome::Stale) => {
            eprintln!("{} is stale; run dx generate --apply", request.lock);
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
