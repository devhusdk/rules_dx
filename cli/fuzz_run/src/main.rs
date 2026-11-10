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
    std::process::exit(run::main_code(std::env::args().skip(1).collect()));
}
