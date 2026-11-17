#![deny(clippy::indexing_slicing, clippy::string_slice)]
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn greeting() -> String {
    "Hello from a Wasm consumer".to_owned()
}

fn main() {
    println!("{}", greeting());
}
