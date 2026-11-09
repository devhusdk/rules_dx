pub mod net;

pub fn join(words: &[&str]) -> String {
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::join;

    #[test]
    fn join_matches_std_join() {
        assert_eq!(join(&["a", "b"]), "a b");
    }
}
