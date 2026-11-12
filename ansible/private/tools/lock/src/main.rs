//! The lock step of the ansible ecosystem.

use std::process::ExitCode;

use ansible_lock::Ansible;

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("ansible") else {
        eprintln!("ansible has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Ansible::spec(hub);
    dx_lock::main(&spec, &Ansible)
}
