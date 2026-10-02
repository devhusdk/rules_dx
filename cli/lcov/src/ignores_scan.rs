use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentStyle {
    SlashSlash,
    Hash,
    Html,
}

fn comment_style(path: &str) -> CommentStyle {
    if path.ends_with(".py")
        || path.ends_with(".pyi")
        || path.ends_with(".bzl")
        || path.ends_with(".toml")
        || path.ends_with(".sh")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
    {
        CommentStyle::Hash
    } else if path.ends_with(".md")
        || path.ends_with(".html")
        || path.ends_with(".htm")
        || path.ends_with(".mdx")
    {
        CommentStyle::Html
    } else {
        CommentStyle::SlashSlash
    }
}

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

fn line_comment(line: &str) -> Option<&str> {
    scan_with(line, slash_scan()).map(|end| &line[end..])
}

fn hash_comment(line: &str) -> Option<&str> {
    scan_with(line, hash_scan()).map(|end| &line[end..])
}

fn html_comments(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find("<!--") {
        let after = &rest[open + "<!--".len()..];
        match after.find("-->") {
            Some(close) => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&after[..close]);
                rest = &after[close + "-->".len()..];
            }
            None => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(after);
                break;
            }
        }
    }
    out
}

pub(crate) fn comment_text(path: &str, line: &str) -> String {
    match comment_style(path) {
        CommentStyle::SlashSlash => line_comment(line).unwrap_or_default().to_string(),
        CommentStyle::Hash => hash_comment(line).unwrap_or_default().to_string(),
        CommentStyle::Html => html_comments(line),
    }
}

pub(crate) fn take_word(rest: &str, word: &str) -> bool {
    match rest.strip_prefix(word) {
        Some(tail) => !tail.starts_with(|c: char| c == '_' || c.is_alphanumeric()),
        None => false,
    }
}
