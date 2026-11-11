//! Creates the simulator one test run uses.

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(dx_apple_ops::create_test_simulator())
}
