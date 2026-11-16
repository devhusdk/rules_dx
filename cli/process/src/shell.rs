//! POSIX shell quoting.

/// Quotes one word for a POSIX shell, always.
#[must_use]
pub fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Quotes a word for a POSIX shell unless it is plainly safe.
#[must_use]
pub fn word(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|cell| cell.is_ascii_alphanumeric() || "_@%+=:,./-".contains(cell));
    if safe { word.to_owned() } else { quote(word) }
}

/// Joins words into one shell command line, quoting each as needed.
#[must_use]
pub fn join(words: &[String]) -> String {
    words
        .iter()
        .map(|item| word(item))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod shell_tests;
