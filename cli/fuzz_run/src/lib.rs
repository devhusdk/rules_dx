#![deny(
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

mod run;
#[cfg(test)]
mod run_tests;

pub fn main() -> std::process::ExitCode {
    std::process::exit(run::main_code(
        &std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    ))
}
