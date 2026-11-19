#![deny(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::string_slice
)]

fn main() {
    std::process::exit(dx_shim::run::main(std::env::args_os().skip(1).collect()));
}
