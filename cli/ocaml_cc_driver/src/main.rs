//! Runs the C tool behind one `conda-ocaml-*` program name the OCaml compiler calls, or one `dune` step.

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
mod dune;
#[cfg(test)]
mod dune_tests;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((mode, rest)) if mode == "dune" => dune::run_from_args(rest),
        _ => driver::run_from_env(),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("ocaml_cc_driver: {error}");
            std::process::exit(1);
        }
    }
}
