//! Runs one fuzz binary for `dx fuzz` and collects the crash files it leaves.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

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
