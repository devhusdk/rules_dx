#![deny(clippy::indexing_slicing, clippy::string_slice)]
fn main() {
    println!("{}", anyhow::anyhow!("hello lock"));
}
