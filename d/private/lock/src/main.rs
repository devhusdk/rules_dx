//! The lock step of the dub ecosystem.

use std::process::ExitCode;

use dub_lock::{Dub, Registry};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("dub") else {
        eprintln!("dub has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Dub::spec(hub);
    dx_lock::main(&spec, &Dub::new(&spec, &Registry::new()))
}
