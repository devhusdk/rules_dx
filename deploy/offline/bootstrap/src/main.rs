//! Installs Bazel and vendored advisory snapshots from an offline bundle.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match dx_bootstrap::parse_args(&args) {
        Ok(dx_bootstrap::Command::Help) => {
            println!("{}", dx_bootstrap::usage());
            return ExitCode::SUCCESS;
        }
        Ok(dx_bootstrap::Command::Bootstrap(options)) => options,
        Err(detail) => {
            eprintln!("bootstrap-offline: {detail}");
            eprintln!("{}", dx_bootstrap::usage());
            return ExitCode::FAILURE;
        }
    };
    let refusal = dx_bootstrap::run(
        &options,
        std::env::consts::OS,
        std::env::consts::ARCH,
        &dx_bootstrap::today_utc(),
    );
    if refusal.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprintln!("bootstrap-offline: {refusal}");
    ExitCode::FAILURE
}
