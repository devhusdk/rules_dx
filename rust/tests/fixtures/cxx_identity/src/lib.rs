pub fn identity_version() -> &'static str {
    "1.0.200"
}

pub fn add(left: u64, right: u64) -> u64 {
    left.wrapping_add(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_pin() {
        assert_eq!(identity_version(), "1.0.200");
    }

    #[test]
    fn adds_numbers() {
        assert_eq!(add(2, 3), 5);
    }
}
