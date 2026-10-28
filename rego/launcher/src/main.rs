//! Runs `opa test` over the Rego files a `rego_test` target stages.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod opa;
#[cfg(test)]
mod opa_tests;

fn main() -> std::process::ExitCode {
    match opa::run_from_env() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("rego_test: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
