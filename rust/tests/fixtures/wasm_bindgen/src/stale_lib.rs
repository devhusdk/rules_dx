use wasm_bindgen::prelude::*;

pub fn greeting(name: &str) -> String {
    format!("Hello from stale wasm, {name}!")
}

#[wasm_bindgen]
pub fn stale_add(left: u32, right: u32) -> u32 {
    left + right
}
