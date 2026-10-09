#[no_mangle]
pub extern "C" fn dx_wasm_add(left: i32, right: i32) -> i32 {
    left.wrapping_add(right)
}

#[cfg(test)]
mod tests {
    use super::dx_wasm_add;

    #[test]
    fn adds_without_bindgen() {
        assert_eq!(dx_wasm_add(40, 2), 42);
    }
}
