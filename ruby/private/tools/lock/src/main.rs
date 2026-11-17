//! The lock step of the ruby ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use ruby_lock::{Registry, Ruby};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("ruby") else {
        eprintln!("ruby has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Ruby::spec(hub);
    dx_lock::main(&spec, &Ruby::new(&spec, &Registry::new()))
}
