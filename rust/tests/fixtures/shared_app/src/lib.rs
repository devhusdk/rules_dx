use wasm_bindgen::prelude::*;

pub fn greeting(name: &str) -> String {
    format!("Hello from shared core, {name}!")
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    greeting(name)
}

#[wasm_bindgen]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counter {
    value: i32,
}

#[wasm_bindgen]
impl Counter {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Counter {
        Counter { value: 0 }
    }

    pub fn add(&mut self, delta: i32) {
        self.value = self.value.saturating_add(delta);
    }

    pub fn reset(&mut self) {
        self.value = 0;
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn render(&self) -> String {
        format!("shared core count is {}", self.value)
    }
}

impl Counter {
    pub fn set_from_str(&mut self, text: &str) -> Result<(), std::num::ParseIntError> {
        self.value = text.trim().parse()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{greeting, Counter};

    #[test]
    fn greeting_mentions_name() {
        assert!(greeting("native").contains("native"));
    }

    #[test]
    fn counter_adds_and_renders() {
        let mut counter = Counter::new();
        counter.add(40);
        counter.add(2);
        assert_eq!(counter.value(), 42);
        assert_eq!(counter.render(), "shared core count is 42");
    }

    #[test]
    fn counter_reset_and_parse() {
        let mut counter = Counter::new();
        counter.add(7);
        counter.reset();
        assert_eq!(counter.value(), 0);
        counter.set_from_str("  9  ").expect("trims and parses");
        assert_eq!(counter.value(), 9);
    }

    #[test]
    fn counter_rejects_non_numeric_input() {
        let mut counter = Counter::new();
        assert!(
            counter.set_from_str("many").is_err(),
            "text input must fail"
        );
        assert_eq!(counter.value(), 0, "failed parse keeps the old value");
    }

    #[test]
    fn counter_saturates_at_the_bounds() {
        let mut counter = Counter::new();
        counter.add(i32::MAX);
        counter.add(1);
        assert_eq!(counter.value(), i32::MAX);
    }
}
