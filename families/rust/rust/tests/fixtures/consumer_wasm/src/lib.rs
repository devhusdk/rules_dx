pub fn greeting() -> String {
    let words = ["Hello", "from", "an", "opt-in", "wasm", "consumer"];
    words.join(" ")
}

pub fn shout(text: &str) -> String {
    text.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{greeting, shout};

    #[test]
    fn greeting_mentions_wasm() {
        assert!(greeting().contains("wasm"));
    }

    #[test]
    fn shout_uppercases() {
        assert_eq!(shout("wasm"), "WASM");
    }
}
