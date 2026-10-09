use wasm_bindgen::prelude::*;

pub fn greeting(name: &str) -> String {
    format!("Hello from wasm, {name}!")
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    greeting(name)
}

#[wasm_bindgen]
pub fn add(left: u32, right: u32) -> u32 {
    left + right
}

#[wasm_bindgen(inline_js = "export function decorate(text) { return text.toUpperCase(); }")]
extern "C" {
    fn decorate(text: &str) -> String;
}

#[wasm_bindgen]
pub fn shout_greet(name: &str) -> String {
    decorate(&greet(name))
}

#[cfg(test)]
mod tests {
    use super::{add, greeting};

    #[test]
    fn greeting_mentions_name() {
        assert!(greeting("dx").contains("dx"));
    }

    #[test]
    fn add_sums() {
        assert_eq!(add(40, 2), 42);
    }
}
