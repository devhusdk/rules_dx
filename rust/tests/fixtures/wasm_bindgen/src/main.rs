fn main() {
    println!("{}", wasm_hello::greeting("wasm"));
}

#[cfg(test)]
mod browser_tests {
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn greet_roundtrip() {
        assert_eq!(wasm_hello::greeting("browser"), "Hello from wasm, browser!");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn add_roundtrip() {
        assert_eq!(wasm_hello::add(40, 2), 42);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn snippet_roundtrip() {
        assert_eq!(wasm_hello::add_via_snippet(40, 2), 42);
    }
}
