//! Lint fixture with a length comparison to zero.

/// Returns whether `items` is empty.
pub fn empty(items: &[u8]) -> bool {
    items.len() == 0
}
