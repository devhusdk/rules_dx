//! The lock step of the hex ecosystem.

use std::process::ExitCode;

use hex_lock::Hex;

fn main() -> ExitCode {
    dx_lock::main(&Hex::spec(), &Hex)
}
