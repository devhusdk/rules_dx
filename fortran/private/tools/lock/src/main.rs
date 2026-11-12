//! The lock step of the fpm ecosystem.

use std::process::ExitCode;

use fpm_lock::{Fpm, Github};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("fpm") else {
        eprintln!("fpm has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Fpm::spec(hub);
    let github = Github::new();
    dx_lock::main(&spec, &Fpm::new(&spec, &github))
}
