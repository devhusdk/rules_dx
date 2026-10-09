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

#[wasm_bindgen(inline_js = "export function snippet_add(left, right) { return left + right; }")]
extern "C" {
    fn snippet_add(left: u32, right: u32) -> u32;
}

#[wasm_bindgen]
pub fn add_via_snippet(left: u32, right: u32) -> u32 {
    snippet_add(left, right)
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
