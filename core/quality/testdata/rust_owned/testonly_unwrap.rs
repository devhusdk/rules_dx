/// Returns the first byte.
pub fn first(items: &[u8]) -> u8 {
    *items.first().unwrap()
}
