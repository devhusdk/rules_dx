#![deny(
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]

mod tree;
#[cfg(test)]
mod tree_tests;

#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    match tree::run(args) {
        Ok(()) => 0,
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("release_tree: {error}"));
            1
        }
    }
}
