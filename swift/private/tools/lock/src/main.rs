//! The lock step of the swiftpm ecosystem.

use std::process::ExitCode;

use swift_lock::{Github, Swift};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("swiftpm") else {
        eprintln!("swiftpm has no registry entry");
        return ExitCode::from(2);
    };
    let runfiles = std::env::var_os("RUNFILES_DIR").map(std::path::PathBuf::from).or_else(|| {
        let mut name = std::env::current_exe().ok()?.into_os_string();
        name.push(".runfiles");
        Some(name.into())
    });
    let swift = runfiles
        .zip(std::env::var_os("SWIFT"))
        .and_then(|(runfiles, path)| std::path::absolute(std::path::Path::new(&runfiles).join(path)).ok())
        .filter(|path| path.is_file());
    let Some(swift) = swift else {
        eprintln!("the swift command is not in the runfiles");
        return ExitCode::from(2);
    };
    let spec = Swift::spec(hub);
    dx_lock::main(&spec, &Swift::new(swift, &spec, &Github::new()))
}
