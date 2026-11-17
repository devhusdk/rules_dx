use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn implicit_greeting() -> String {
    "Hello from an implicit wasm_bindgen dep".to_owned()
}

fn main() {
    println!("{}", implicit_greeting());
}
