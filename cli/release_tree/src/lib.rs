//! Assembles a directory from `--dir <path>` and `--copy <source> <path>` steps below the output directory.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod tree;
#[cfg(test)]
mod tree_tests;

/// Runs `release_tree` with the arguments after its name and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    match tree::run(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("release_tree: {error}");
            1
        }
    }
}
