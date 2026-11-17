//! The lock step of the pub ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::PathBuf;
use std::process::ExitCode;

use pub_lock::{Outcome, Request, lock};

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
    let Some(dir) = dir.filter(|dir| dir.is_absolute()) else {
        return usage();
    };
    let Some(hub) = dx_families::hub("pub") else {
        eprintln!("pub has no registry entry");
        return ExitCode::from(2);
    };
    let Ok(runfiles) = runfiles::Runfiles::create() else {
        eprintln!("pub lock: no runfiles");
        return ExitCode::from(2);
    };
    let Some(dart) = std::env::var("DX_DART")
        .ok()
        .and_then(|path| runfiles::rlocation!(runfiles, path))
    else {
        eprintln!("pub lock: the pinned dart is not in runfiles");
        return ExitCode::from(2);
    };
    let request = Request {
        dir,
        manifest: hub.manifest.clone(),
        lock: hub.lock.clone().unwrap_or_default(),
        check,
    };
    match lock(&dart, &request) {
        Ok(Outcome::Fresh | Outcome::Written) => ExitCode::SUCCESS,
        Ok(Outcome::Stale) => {
            eprintln!("{} is stale; run dx generate --apply", request.lock);
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{}: {error}", request.lock);
            ExitCode::from(2)
        }
    }
}
