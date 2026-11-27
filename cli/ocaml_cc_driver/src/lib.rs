#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::ffi::{OsStr, OsString};
use std::path::Path;

mod driver;
#[cfg(test)]
mod driver_tests;
mod dune;
#[cfg(test)]
mod dune_tests;
#[cfg(test)]
mod lib_tests;

#[must_use]
pub fn is_driver(argv0: &OsStr) -> bool {
    Path::new(argv0)
        .file_stem()
        .and_then(OsStr::to_str)
        .and_then(|stem| stem.strip_prefix(driver::PREFIX))
        .is_some_and(|tool| driver::TOOLS.contains(&tool))
}

#[must_use]
pub fn run_tool(argv0: &OsStr, args: Vec<OsString>) -> i32 {
    exit_code(driver::run(argv0, args))
}

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
