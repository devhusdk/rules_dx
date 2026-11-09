pub fn greeting() -> String {
    let words = ["Hello", "from", "a", "std-only", "consumer"];
    words.join(" ")
}

pub fn shout(text: &str) -> String {
    text.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{greeting, shout};

    #[test]
    fn greets_through_std_only() {
        assert_eq!(greeting(), "Hello from a std-only consumer");
    }

    #[test]
    fn shouts_with_std_only() {
        assert_eq!(shout("hi"), "HI");
    }
}
