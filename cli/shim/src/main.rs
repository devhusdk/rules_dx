#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

fn main() {
    std::process::exit(dx_shim::run::main(std::env::args_os().skip(1).collect()));
}
