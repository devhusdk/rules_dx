#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! Prints the greeting.

fn main() {
    println!("{}", conformance::greet("world"));
}
