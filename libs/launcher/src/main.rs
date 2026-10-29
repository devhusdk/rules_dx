//! Runs one program with the arguments a `<name>.launch` descriptor lists.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod launch;
#[cfg(test)]
mod launch_tests;

fn main() {
    match launch::run_from_env() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("exec_launcher: {error}");
            std::process::exit(1);
        }
    }
}
