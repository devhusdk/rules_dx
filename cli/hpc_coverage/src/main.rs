//! Runs a Haskell test and, under coverage, converts the hpc data it wrote to LCOV.
//!
//! Arguments: `<test> <hpc_lcov> [<mix file>...] -- [<test argument>...]`.

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

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match run::run(&args, &|name| std::env::var_os(name)) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("hpc_coverage: {error}"));
            std::process::exit(1);
        }
    }
}
