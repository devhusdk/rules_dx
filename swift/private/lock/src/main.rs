//! The lock step of the swiftpm ecosystem.

use std::process::ExitCode;

use swift_lock::{Github, Swift};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("swiftpm") else {
        eprintln!("swiftpm has no registry entry");
        return ExitCode::from(2);
    };
    let swift = std::env::var_os("RUNFILES_DIR")
        .zip(std::env::var_os("SWIFT"))
        .and_then(|(runfiles, path)| std::fs::canonicalize(std::path::Path::new(&runfiles).join(path)).ok());
    let Some(swift) = swift else {
        eprintln!("the swift command is not in the runfiles");
        return ExitCode::from(2);
    };
    let spec = Swift::spec(hub);
    dx_lock::main(&spec, &Swift::new(swift, &spec, &Github::new()))
}
