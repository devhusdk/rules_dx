use regex::Regex;
use std::sync::LazyLock;

use crate::verdict::{line_comment_syntax, LineComment};

fn slash_scan() -> Option<&'static Regex> {
    static SCAN: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|(?P<marker>//)"#).ok());
    SCAN.as_ref()
}

fn hash_scan() -> Option<&'static Regex> {
    static SCAN: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|(?P<marker>#)"#).ok());
    SCAN.as_ref()
}

fn scan_with(line: &str, compiled: Option<&Regex>) -> Option<usize> {
    let re = compiled?;
    for captures in re.captures_iter(line) {
        if let Some(marker) = captures.name("marker") {
            return Some(marker.start() + marker.as_str().len());
        }
    }
    None
}

fn slash_comment(line: &str) -> Option<&str> {
    scan_with(line, slash_scan()).map(|end| &line[end..])
}

fn hash_comment(line: &str) -> Option<&str> {
    scan_with(line, hash_scan()).map(|end| &line[end..])
}

pub(crate) fn comment_text(path: &str, line: &str) -> String {
    match line_comment_syntax(path) {
        Some(LineComment::SlashSlash) => slash_comment(line).unwrap_or_default().to_string(),
        Some(LineComment::Hash) => hash_comment(line).unwrap_or_default().to_string(),
        None => String::new(),
    }
}

pub(crate) fn take_word(rest: &str, word: &str) -> bool {
    match rest.strip_prefix(word) {
        Some(tail) => !tail.starts_with(|c: char| c == '_' || c.is_alphanumeric()),
        None => false,
    }
}
