#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

fn main() {
    std::process::exit(dx_shim::run::main(std::env::args_os().skip(1).collect()));
}
