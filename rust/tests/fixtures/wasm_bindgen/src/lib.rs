use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn dx_greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn greets_by_name() {
        assert_eq!(super::dx_greet("wasm"), "Hello, wasm!");
    }
}
