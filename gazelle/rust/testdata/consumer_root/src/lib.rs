//! The smallest crate a user could adopt dx with; release qualification builds it with the
//! extracted dx.

/// Adds two numbers.
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::add;

    #[test]
    fn adds() {
        assert_eq!(add(2, 3), 5);
    }
}
