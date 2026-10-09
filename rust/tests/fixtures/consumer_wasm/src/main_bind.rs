use wasm_bindgen::prelude::*;

pub fn host_triple(value: i32) -> i32 {
    value * 3
}

#[wasm_bindgen]
pub fn wasm_triple(value: i32) -> i32 {
    host_triple(value)
}

fn main() {
    println!("{}", host_triple(14));
    println!("{}", wasm_triple(14));
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn tripling_in_browser() {
        assert_eq!(super::host_triple(14), 42);
    }

    #[wasm_bindgen_test]
    fn exported_tripling_in_browser() {
        assert_eq!(super::wasm_triple(14), 42);
    }
}
