fn main() {
    let mut counter = shared_app::Counter::new();
    counter.add(40);
    counter.add(2);
    println!("{}", shared_app::greeting("native"));
    println!("{}", counter.render());
}

#[cfg(test)]
mod browser_tests {
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn greet_roundtrip() {
        assert_eq!(
            shared_app::greeting("browser"),
            "Hello from shared core, browser!"
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn counter_roundtrip() {
        let mut counter = shared_app::Counter::new();
        counter.add(40);
        counter.add(2);
        assert_eq!(counter.value(), 42);
        assert_eq!(counter.render(), "shared core count is 42");
    }
}
