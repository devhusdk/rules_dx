//! `module_pin_bin [MODULE.bazel]` checks the rules_dx version against `RULES_DX_PIN`.

use std::process::ExitCode;

fn main() -> ExitCode {
    if let Some(dir) = std::env::var_os("BUILD_WORKING_DIRECTORY") {
        if let Err(error) = std::env::set_current_dir(&dir) {
            eprintln!("dx-ci: cannot enter {dir:?}: {error}");
            return ExitCode::FAILURE;
        }
    }
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "MODULE.bazel".to_string());
    let pin = std::env::var("RULES_DX_PIN").unwrap_or_default();
    match module_pin::verify(&path, &pin) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("dx-ci: {message}");
            ExitCode::FAILURE
        }
    }
}
