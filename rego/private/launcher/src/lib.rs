//! Runs `opa test` over the Rego files a `rego_test` target stages.
#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

mod opa;
#[cfg(test)]
mod opa_tests;

/// Runs the `rego_test` named by the process environment.
pub fn main() -> std::process::ExitCode {
    match opa::run_from_env() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("rego_test: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
