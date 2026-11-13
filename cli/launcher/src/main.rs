//! Runs one program with the arguments a `<name>.launch` descriptor lists.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

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
