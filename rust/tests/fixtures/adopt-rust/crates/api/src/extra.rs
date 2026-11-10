use super::digest;

pub fn digest_pair(first: &str, second: &str) -> u64 {
    digest((first, second))
}

#[cfg(test)]
mod tests {
    use super::digest_pair;

    #[test]
    fn pair_is_order_sensitive() {
        assert_ne!(digest_pair("a", "b"), digest_pair("b", "a"));
    }
}
