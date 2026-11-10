//! Runs one fuzz binary for `dx fuzz` and collects the crash files it leaves.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod run;
#[cfg(test)]
mod run_tests;

fn main() {
    std::process::exit(run::main_code(
        std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
    ));
}
