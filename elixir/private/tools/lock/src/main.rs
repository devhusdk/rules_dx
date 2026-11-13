//! The lock step of the hex ecosystem.

use std::process::ExitCode;

use hex_lock::Hex;

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("hex") else {
        eprintln!("hex has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Hex::spec(hub);
    dx_lock::main(&spec, &Hex::new(&spec))
}
