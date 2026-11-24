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

mod launch;
#[cfg(test)]
mod launch_tests;

fn main() {
    match launch::run_from_env() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("exec_launcher: {error}"));
            std::process::exit(1);
        }
    }
}
