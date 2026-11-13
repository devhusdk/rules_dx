//! The lock step of the zig ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use zig_lock::{Github, Zig};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("zig") else {
        eprintln!("zig has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Zig::spec(hub);
    dx_lock::main(&spec, &Zig::new(&spec, &Github::new()))
}
