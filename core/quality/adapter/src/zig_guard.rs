//! Banned Zig patterns that the strict tier fails on. Comments and string contents never match.

use std::collections::{BTreeMap, BTreeSet};

use crate::parsers::FileFinding;
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "zig_ast_check";

/// Casts that need a `// SAFETY:` comment in or above their statement.
const CHECKED_CASTS: &[&str] = &["@ptrCast", "@alignCast", "@intCast"];

const SAFETY: &str = "SAFETY:";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// An identifier, keyword, number or `@"quoted"` identifier.
    Word,
    /// A builtin name such as `@intCast`.
    Builtin,
    /// A string, `\\` line string or char literal, quotes included.
    Text,
    /// One punctuation byte.
    Punct,
}

struct Token<'a> {
    kind: Kind,
    text: &'a str,
    line: u64,
    column: u64,
}

struct Lexed<'a> {
    tokens: Vec<Token<'a>>,
    /// The `//` comment on each line that has one.
    comments: BTreeMap<u64, &'a str>,
    /// Lines that hold at least one token.
    code_lines: BTreeSet<u64>,
}

/// Returns a finding for each banned pattern in one Zig source file.
#[must_use]
pub fn zig_guard(file: &str, source: &str) -> Vec<FileFinding> {
    let lexed = lex(source);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut test_depth: Option<usize> = None;
    let mut test_pending = false;
    let mut statement_line: Option<u64> = None;
    for (i, token) in lexed.tokens.iter().enumerate() {
        let start = *statement_line.get_or_insert(token.line);
        let rest = lexed.tokens.get(i + 1..).unwrap_or_default();
        let hit = match (token.kind, token.text) {
            (Kind::Word, "test") if test_depth.is_none() => {
                test_pending = true;
                None
            }
            (Kind::Word, "catch") => catch_rule(rest),
            (Kind::Word, "std")
                if test_depth.is_none() && is_seq(rest, &[".", "debug", ".", "print"]) =>
            {
                Some((
                    "debug_print",
                    "`std.debug.print` outside a test; write to a writer or use `std.log`"
                        .to_owned(),
                ))
            }
            (Kind::Builtin, "@panic") if is_todo(rest) => Some((
                "todo_panic",
                "`@panic(\"TODO...\")` marks unfinished code".to_owned(),
            )),
            (Kind::Builtin, cast)
                if CHECKED_CASTS.contains(&cast) && !has_safety(&lexed, start, token.line) =>
            {
                Some((
                    "unsafe_cast",
                    format!("`{cast}` needs a `// SAFETY:` comment that says why it holds"),
                ))
            }
            _ => None,
        };
        if token.kind == Kind::Punct {
            match token.text {
                "{" => {
                    depth += 1;
                    if test_pending {
                        test_depth = Some(depth);
                        test_pending = false;
                    }
                }
                "}" => {
                    if test_depth == Some(depth) {
                        test_depth = None;
                    }
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            if matches!(token.text, ";" | "{" | "}") {
                statement_line = None;
            }
        }
        if let Some((rule, message)) = hit {
            out.push(finding(file, token, rule, message));
        }
    }
    out
}

/// `catch unreachable` or `catch {}`, with or without a `|err|` payload.
fn catch_rule(rest: &[Token<'_>]) -> Option<(&'static str, String)> {
    let mut rest = rest;
    if is_punct(rest.first(), "|") {
        let close = rest.iter().skip(1).position(|t| is_punct(Some(t), "|"))?;
        rest = rest.get(close + 2..)?;
    }
    let first = rest.first()?;
    if first.kind == Kind::Word && first.text == "unreachable" {
        return Some((
            "catch_unreachable",
            "`catch unreachable` turns an error into undefined behaviour; handle it or return it with `try`".to_owned(),
        ));
    }
    if is_punct(Some(first), "{") && is_punct(rest.get(1), "}") {
        return Some((
            "empty_catch",
            "`catch {}` throws the error away; handle it or return it with `try`".to_owned(),
        ));
    }
    None
}

/// `@panic("TODO...")`.
fn is_todo(rest: &[Token<'_>]) -> bool {
    is_punct(rest.first(), "(")
        && rest
            .get(1)
            .is_some_and(|t| t.kind == Kind::Text && t.text.starts_with("\"TODO"))
}

/// Whether the tokens start with exactly these texts.
fn is_seq(rest: &[Token<'_>], texts: &[&str]) -> bool {
    rest.len() >= texts.len() && rest.iter().zip(texts).all(|(t, want)| t.text == *want)
}

fn is_punct(token: Option<&Token<'_>>, text: &str) -> bool {
    token.is_some_and(|t| t.kind == Kind::Punct && t.text == text)
}

/// A `// SAFETY:` comment on a line of the statement, or in the comment-only lines right above it.
fn has_safety(lexed: &Lexed<'_>, start: u64, line: u64) -> bool {
    if lexed
        .comments
        .range(start..=line)
        .any(|(_, text)| text.contains(SAFETY))
    {
        return true;
    }
    let mut above = start;
    while above > 1 {
        above -= 1;
        let Some(text) = lexed.comments.get(&above) else {
            return false;
        };
        if lexed.code_lines.contains(&above) {
            return false;
        }
        if text.contains(SAFETY) {
            return true;
        }
    }
    false
}

fn finding(file: &str, token: &Token<'_>, rule: &str, message: String) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message,
            severity: ToolSeverity::Error,
            start: TextPosition {
                line: token.line,
                column: token.column,
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Utf8,
            related: Vec::new(),
        },
    }
}

fn lex(source: &str) -> Lexed<'_> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut comments = BTreeMap::new();
    let (mut i, mut line, mut line_start) = (0usize, 1u64, 0usize);
    while let Some(&b) = bytes.get(i) {
        let start = i;
        let next = bytes.get(i + 1).copied();
        let kind = match b {
            b'\n' => {
                i += 1;
                line += 1;
                line_start = i;
                continue;
            }
            b' ' | b'\t' | b'\r' => {
                i += 1;
                continue;
            }
            b'/' if next == Some(b'/') => {
                i = line_end(bytes, i);
                comments.insert(line, source.get(start..i).unwrap_or_default());
                continue;
            }
            b'\\' if next == Some(b'\\') => {
                i = line_end(bytes, i);
                Kind::Text
            }
            b'"' | b'\'' => {
                i = quoted_end(bytes, i, b);
                Kind::Text
            }
            b'@' if next == Some(b'"') => {
                i = quoted_end(bytes, i + 1, b'"');
                Kind::Word
            }
            b'@' if next.is_some_and(is_word_byte) => {
                i = word_end(bytes, i + 1);
                Kind::Builtin
            }
            _ if is_word_byte(b) => {
                i = word_end(bytes, i);
                Kind::Word
            }
            _ => {
                i += 1;
                Kind::Punct
            }
        };
        tokens.push(Token {
            kind,
            text: source.get(start..i).unwrap_or_default(),
            line,
            column: u64::try_from(start - line_start).unwrap_or(u64::MAX) + 1,
        });
    }
    let code_lines = tokens.iter().map(|t| t.line).collect();
    Lexed {
        tokens,
        comments,
        code_lines,
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn word_end(bytes: &[u8], from: usize) -> usize {
    from + bytes
        .get(from..)
        .unwrap_or_default()
        .iter()
        .take_while(|b| is_word_byte(**b))
        .count()
}

fn line_end(bytes: &[u8], from: usize) -> usize {
    from + bytes
        .get(from..)
        .unwrap_or_default()
        .iter()
        .take_while(|b| **b != b'\n')
        .count()
}

/// The index after the closing quote. An unclosed literal ends before the newline, as Zig never lets one span lines.
fn quoted_end(bytes: &[u8], open: usize, quote: u8) -> usize {
    let mut j = open + 1;
    loop {
        match bytes.get(j) {
            None | Some(b'\n') => return j,
            Some(b'\\') => {
                j += 1;
                if bytes.get(j).is_some_and(|b| *b != b'\n') {
                    j += 1;
                }
            }
            Some(b) if *b == quote => return j + 1,
            Some(_) => j += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(source: &str) -> Vec<(String, u64, u64)> {
        zig_guard("a.zig", source)
            .into_iter()
            .map(|f| {
                (
                    f.finding.rule_id,
                    f.finding.start.line,
                    f.finding.start.column,
                )
            })
            .collect()
    }

    fn hits(want: &[(&str, u64, u64)]) -> Vec<(String, u64, u64)> {
        want.iter()
            .map(|(rule, line, column)| ((*rule).to_owned(), *line, *column))
            .collect()
    }

    fn one(rule: &str, line: u64, column: u64) -> Vec<(String, u64, u64)> {
        hits(&[(rule, line, column)])
    }

    #[test]
    fn clean_source_has_no_findings() {
        let source = "const std = @import(\"std\");\n\
                      pub fn f(x: u32) !u8 {\n\
                      \x20   // SAFETY: callers pass values below 256.\n\
                      \x20   const y: u8 = @intCast(x);\n\
                      \x20   const z = g() catch |err| return err;\n\
                      \x20   std.log.info(\"{d}\", .{z});\n\
                      \x20   return y;\n\
                      }\n\
                      test \"prints\" {\n\
                      \x20   std.debug.print(\"ok\\n\", .{});\n\
                      }\n";
        assert_eq!(rules(source), hits(&[]));
    }

    #[test]
    fn catch_unreachable_is_found_with_and_without_a_payload() {
        assert_eq!(
            rules("const a = f() catch unreachable;\n"),
            one("catch_unreachable", 1, 15)
        );
        assert_eq!(
            rules("const a = f() catch |_| unreachable;\n"),
            one("catch_unreachable", 1, 15)
        );
    }

    #[test]
    fn empty_catch_is_found() {
        assert_eq!(rules("f() catch {};\n"), one("empty_catch", 1, 5));
        assert_eq!(rules("f() catch |_| {\n};\n"), one("empty_catch", 1, 5));
        assert_eq!(rules("f() catch |e| {\n    log(e);\n};\n"), hits(&[]));
    }

    #[test]
    fn debug_print_is_found_outside_tests_only() {
        let source = "fn f() void {\n    std.debug.print(\"x\", .{});\n}\n\
                      test {\n    if (true) {\n        std.debug.print(\"x\", .{});\n    }\n}\n\
                      fn g() void {\n    std.debug.print(\"y\", .{});\n}\n";
        assert_eq!(
            rules(source),
            hits(&[("debug_print", 2, 5), ("debug_print", 10, 5)])
        );
    }

    #[test]
    fn todo_panic_is_found() {
        assert_eq!(rules("@panic(\"TODO: later\");\n"), one("todo_panic", 1, 1));
        assert_eq!(rules("@panic(\"bad state\");\n"), hits(&[]));
    }

    #[test]
    fn unsafe_cast_needs_a_safety_comment() {
        assert_eq!(
            rules("const p: *u32 = @ptrCast(@alignCast(q));\n"),
            hits(&[("unsafe_cast", 1, 17), ("unsafe_cast", 1, 26)])
        );
        assert_eq!(
            rules("const p: *u32 = @ptrCast(@alignCast(q)); // SAFETY: q is a *u32.\n"),
            hits(&[])
        );
        let above = "// SAFETY: n fits.\n// More context.\nconst x: u8 =\n    @intCast(n);\n";
        assert_eq!(rules(above), hits(&[]));
        let far = "// SAFETY: n fits.\nconst a = 1;\nconst x: u8 = @intCast(n);\n";
        assert_eq!(rules(far), one("unsafe_cast", 3, 15));
    }

    #[test]
    fn comments_and_strings_never_match() {
        let source = "// catch unreachable and std.debug.print and @intCast\n\
                      const s = \"catch {} @intCast std.debug.print\";\n\
                      const m =\n    \\\\ @panic(\"TODO\") catch unreachable\n;\n\
                      const c = '\"';\n";
        assert_eq!(rules(source), hits(&[]));
    }

    #[test]
    fn columns_are_utf8_bytes() {
        assert_eq!(
            rules("const s = \"é\"; const a = f() catch unreachable;\n"),
            one("catch_unreachable", 1, 31)
        );
    }

    #[test]
    fn unclosed_literals_stop_at_the_line_end() {
        assert_eq!(
            rules("const s = \"open\nf() catch {};\n"),
            one("empty_catch", 2, 5)
        );
    }
}
