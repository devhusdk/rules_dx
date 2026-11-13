//! The lock step of the bicep ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use bicep_lock::{Bicep, Site};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("bicep") else {
        eprintln!("bicep has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Bicep::spec(hub);
    dx_lock::main(&spec, &Bicep::new(&Site::new()))
}
