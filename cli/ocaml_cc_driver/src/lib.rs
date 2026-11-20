//! Runs the C tool behind one `conda-ocaml-*` program name the OCaml compiler calls, or one `dune` step.

#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::ffi::{OsStr, OsString};

mod driver;
#[cfg(test)]
mod driver_tests;
mod dune;
#[cfg(test)]
mod dune_tests;
#[cfg(test)]
mod lib_tests;

/// Runs the `conda-ocaml-*` tool named by `argv0` with `args` and returns its exit code.
#[must_use]
pub fn run_tool(argv0: &OsStr, args: Vec<OsString>) -> i32 {
    exit_code(driver::run(argv0, args))
}

/// Runs the dune step named by `args` (MODE ARG...) and returns its exit code.
#[must_use]
pub fn run_dune(args: &[String]) -> i32 {
    exit_code(dune::run_from_args(args))
}

fn exit_code(result: Result<i32, String>) -> i32 {
    result.unwrap_or_else(|error| {
        eprintln!("ocaml_cc_driver: {error}");
        1
    })
}
