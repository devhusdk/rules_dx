#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The lock step of the cabal ecosystem.

use std::path::PathBuf;
use std::process::ExitCode;

use cabal_lock::Cabal;
use serde_json as _;
use tar as _;

fn runfile(runfiles: &runfiles::Runfiles, variable: &str) -> Option<PathBuf> {
    let path = std::env::var(variable).ok()?;
    runfiles::rlocation!(runfiles, path)
}

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("cabal") else {
        eprintln!("cabal has no registry entry");
        return ExitCode::from(2);
    };
    let Ok(runfiles) = runfiles::Runfiles::create() else {
        eprintln!("cabal lock: no runfiles");
        return ExitCode::from(2);
    };
    let (Some(cabal), Some(ghc)) = (runfile(&runfiles, "DX_CABAL"), runfile(&runfiles, "DX_GHC"))
    else {
        eprintln!("cabal lock: the pinned cabal or ghc is not in runfiles");
        return ExitCode::from(2);
    };
    dx_lock::main(&Cabal::spec(hub), &Cabal { cabal, ghc })
}
