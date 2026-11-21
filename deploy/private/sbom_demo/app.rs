#![deny(
    clippy::indexing_slicing,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok
)]

fn main() {
    println!("{}", greeter::greeting());
}
