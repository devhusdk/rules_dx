//! The lock step of the powershell ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use powershell_lock::Powershell;

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("powershell") else {
        eprintln!("powershell has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Powershell::spec(hub);
    dx_lock::main(&spec, &Powershell)
}
