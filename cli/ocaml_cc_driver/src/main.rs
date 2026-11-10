//! Runs the C tool behind one `conda-ocaml-*` program name the OCaml compiler calls.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod driver;
#[cfg(test)]
mod driver_tests;

fn main() {
    match driver::run_from_env() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("ocaml_cc_driver: {error}");
            std::process::exit(1);
        }
    }
}
