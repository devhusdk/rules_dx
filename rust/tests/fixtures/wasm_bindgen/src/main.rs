fn main() {
    println!("{}", wasm_hello::greeting("wasm"));
}

#[cfg(all(test, target_arch = "wasm32"))]
mod browser_tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn greet_roundtrip() {
        assert_eq!(
            wasm_hello::greeting("browser"),
            "Hello from wasm, browser!"
        );
    }

    #[wasm_bindgen_test]
    fn add_roundtrip() {
        assert_eq!(wasm_hello::add(40, 2), 42);
    }
}
