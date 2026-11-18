//! Runs one fuzz binary for `dx fuzz` and collects the crash files it leaves.

#![deny(
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]

mod run;
#[cfg(test)]
mod run_tests;

/// Runs the fuzz binary named by the command line and exits with its code.
pub fn main() -> std::process::ExitCode {
    std::process::exit(run::main_code(
        &std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    ))
}
