//! The lock step of the terraform ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use terraform_lock::{Terraform, Site};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("terraform") else {
        eprintln!("terraform has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Terraform::spec(hub);
    dx_lock::main(&spec, &Terraform::new(&Site::new(), hub))
}
