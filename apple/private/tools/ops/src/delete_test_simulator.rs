//! Deletes the simulator one test run used.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(dx_apple_ops::delete_test_simulator())
}
