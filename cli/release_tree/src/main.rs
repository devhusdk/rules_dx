//! Assembles a directory from `--dir <path>` and `--copy <source> <path>` steps below the output directory.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod tree;
#[cfg(test)]
mod tree_tests;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match tree::run(&args) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("release_tree: {error}");
            std::process::exit(1);
        }
    }
}
