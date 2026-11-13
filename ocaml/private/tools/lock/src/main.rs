#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The lock step of the opam ecosystem.

use std::process::ExitCode;

use opam_lock::{Https, Opam, REPOSITORY};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("opam") else {
        eprintln!("opam has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Opam::spec(hub);
    dx_lock::main(&spec, &Opam::new(REPOSITORY, &Https::new()))
}
