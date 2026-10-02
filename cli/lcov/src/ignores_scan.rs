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

fn directive_suffix() -> Option<&'static Regex> {
    static SUFFIX: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"^_(LINE|START|STOP)\b").ok());
    SUFFIX.as_ref()
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

fn line_comment_with<'a>(line: &'a str, opener: &[u8]) -> Option<&'a str> {
    if opener == b"//" {
        if let Some(end) = scan_with(line, slash_scan()) {
            return Some(&line[end..]);
        }
        if slash_scan().is_some() {
            return None;
        } // LCOV_EXCL_LINE - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    } else if opener == b"#" {
        if let Some(end) = scan_with(line, hash_scan()) {
            return Some(&line[end..]);
        }
        if hash_scan().is_some() {
            return None;
        } // LCOV_EXCL_LINE - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    } // LCOV_EXCL_LINE - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    line_comment_with_fallback(line, opener) // LCOV_EXCL_LINE - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
}

// LCOV_EXCL_START - reason: compile-fail fallback is unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn line_comment_with_fallback<'a>(line: &'a str, opener: &[u8]) -> Option<&'a str> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if in_char {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'\'' {
                in_char = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'\'' {
            in_char = true;
        } else if bytes[index..].starts_with(opener) {
            return Some(&line[index + opener.len()..]);
        }
        index += 1;
    }
    None
}
// LCOV_EXCL_STOP - reason: end compile-fail fallback, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn line_comment(line: &str) -> Option<&str> {
    line_comment_with(line, b"//")
}

fn hash_comment(line: &str) -> Option<&str> {
    line_comment_with(line, b"#")
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
    if let Some(re) = directive_suffix() {
        return match re.find(rest) {
            Some(matched) => matched.as_str() == word,
            None => false,
        };
    } // LCOV_EXCL_LINE - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
      // LCOV_EXCL_START - reason: fallback handles compile-fail path, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    if let Some(tail) = rest.strip_prefix(word) {
        !tail.starts_with(|c: char| c == '_' || c.is_alphanumeric())
    } else {
        false
    }
    // LCOV_EXCL_STOP - reason: end compile-fail fallback, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
}
