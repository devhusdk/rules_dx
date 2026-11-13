//! The lock step of the composer ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use php_lock::{Packagist, Php};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("composer") else {
        eprintln!("composer has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Php::spec(hub);
    dx_lock::main(&spec, &Php::new(&spec, &Packagist::new()))
}
