//! The lock step of the nuget ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use nuget_lock::{Nuget, Registry};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("nuget") else {
        eprintln!("nuget has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Nuget::spec(hub);
    dx_lock::main(&spec, &Nuget::new(&spec, &Registry::new()))
}
