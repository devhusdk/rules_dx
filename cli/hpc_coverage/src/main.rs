//! Runs a Haskell test and, under coverage, converts the hpc data it wrote to LCOV.
//!
//! Arguments: `<test> <hpc_lcov> [<test argument>...]`.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod run;
#[cfg(test)]
mod run_tests;

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match run::run(&args, &|name| std::env::var_os(name)) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("hpc_coverage: {error}");
            std::process::exit(1);
        }
    }
}
