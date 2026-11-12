#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The lock step of the cpan ecosystem.

use std::process::ExitCode;

use perl_lock::Cpan;

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("cpan") else {
        eprintln!("cpan has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Cpan::spec(hub);
    dx_lock::main(&spec, &Cpan)
}
