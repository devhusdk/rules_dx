//! Runs the Android packaging operations.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(dx_android_ops::run(&args))
}
