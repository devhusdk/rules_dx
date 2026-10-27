fn main() {
    println!("{}", consumer_web::greeting("native"));
}

#[cfg(test)]
mod browser_tests {
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn greeting_roundtrip() {
        assert_eq!(consumer_web::greeting("browser"), "Hello from browser!");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn add_roundtrip() {
        assert_eq!(consumer_web::add(40, 2), 42);
    }
}
