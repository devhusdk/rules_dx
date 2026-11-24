use std::collections::{BTreeMap, BTreeSet};

use crate::parsers::FileFinding;
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "zig_ast_check";

const CHECKED_CASTS: &[&str] = &["@ptrCast", "@alignCast", "@intCast"];

const SAFETY: &str = "SAFETY:";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Builtin,
    Text,
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
    comments: BTreeMap<u64, &'a str>,
    code_lines: BTreeSet<u64>,
}

const TYPE_KEYWORDS: &[&str] = &["error", "struct", "enum", "union", "opaque"];

const BLOCK_KEYWORDS: &[&str] = &[
    "if",
    "while",
    "for",
    "else",
    "defer",
    "errdefer",
    "comptime",
    "nosuspend",
    "suspend",
    "struct",
    "enum",
    "union",
    "opaque",
    "catch",
    "orelse",
    "test",
];

struct Frame {
    start: u64,
    header_fn: bool,
    expression: bool,
    continues: bool,
}

#[must_use]
pub fn zig_guard(file: &str, source: &str) -> Vec<FileFinding> {
    let lexed = lex(source);
    let names = StdNames::collect(&lexed.tokens);
    let mut out = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut parens: Vec<usize> = Vec::new();
    let mut last_paren_open: Option<usize> = None;
    let mut test_depth: Option<usize> = None;
    let mut test_pending = false;
    let mut statement_line: Option<u64> = None;
    let mut statement_fn = false;
    for (i, token) in lexed.tokens.iter().enumerate() {
        let start = *statement_line.get_or_insert(token.line);
        let rest = lexed.tokens.get(i + 1..).unwrap_or_default();
        let hit = match (token.kind, token.text) {
            (Kind::Word, "test") if test_depth.is_none() => {
                test_pending = true;
                None
            }
            (Kind::Word, "fn") => {
                statement_fn = true;
                None
            }
            (Kind::Word, "catch") => catch_rule(rest),
            _ if test_depth.is_none() && names.is_debug_print(&lexed.tokens, i) => Some((
                "debug_print",
                "`std.debug.print` outside a test; write to a writer or use `std.log`".to_owned(),
            )),
            (Kind::Builtin, "@panic") if is_todo(rest) => Some((
                "todo_panic",
                "`@panic(\"TODO...\")` marks unfinished code".to_owned(),
            )),
            (Kind::Builtin, cast)
                if CHECKED_CASTS.contains(&cast)
                    && !cast_documented(&lexed, &frames, start, token.line) =>
            {
                Some((
                    "unsafe_cast",
                    format!("`{cast}` needs a `// SAFETY:` comment that says why it holds"),
                ))
            }
            _ => None,
        };
        if let Some((rule, message)) = hit {
            out.push(finding(file, token, rule, message));
        }
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => parens.push(i),
            ")" => last_paren_open = parens.pop(),
            "{" => {
                let head = brace_head(&lexed.tokens, i, last_paren_open);
                let expression = opens_expression(head, statement_fn);
                let opens_type =
                    head.is_some_and(|h| h.kind == Kind::Word && TYPE_KEYWORDS.contains(&h.text));
                let in_header = statement_fn && !opens_fn_body(head);
                frames.push(Frame {
                    start,
                    header_fn: statement_fn,
                    expression,
                    continues: expression || opens_type || in_header || !parens.is_empty(),
                });
                if test_pending {
                    test_depth = Some(frames.len());
                    test_pending = false;
                }
                statement_line = None;
                statement_fn = false;
            }
            "}" => {
                if test_depth == Some(frames.len()) {
                    test_depth = None;
                }
                let outer = frames.pop().filter(|f| f.continues);
                statement_line = outer.as_ref().map(|f| f.start);
                statement_fn = outer.is_some_and(|f| f.header_fn);
            }
            ";" => {
                statement_line = None;
                statement_fn = false;
            }
            _ => {}
        }
    }
    out
}

fn catch_rule(rest: &[Token<'_>]) -> Option<(&'static str, String)> {
    let mut rest = rest;
    if is_punct(rest.first(), "|") {
        let close = rest.iter().skip(1).position(|t| is_punct(Some(t), "|"))?;
        rest = rest.get(close + 2..)?;
    }
    let first = rest.first()?;
    let unreachable_body = is_punct(Some(first), "{")
        && is_word(rest.get(1), "unreachable")
        && (is_punct(rest.get(2), "}")
            || (is_punct(rest.get(2), ";") && is_punct(rest.get(3), "}")));
    if is_word(Some(first), "unreachable") || unreachable_body {
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

fn is_todo(rest: &[Token<'_>]) -> bool {
    is_punct(rest.first(), "(")
        && rest
            .get(1)
            .is_some_and(|t| t.kind == Kind::Text && t.text.starts_with("\"TODO"))
}

fn is_punct(token: Option<&Token<'_>>, text: &str) -> bool {
    token.is_some_and(|t| t.kind == Kind::Punct && t.text == text)
}

fn is_word(token: Option<&Token<'_>>, text: &str) -> bool {
    token.is_some_and(|t| t.kind == Kind::Word && t.text == text)
}

fn brace_head<'t, 'a>(
    tokens: &'t [Token<'a>],
    i: usize,
    paren_open: Option<usize>,
) -> Option<&'t Token<'a>> {
    let prev = tokens.get(i.checked_sub(1)?)?;
    if is_punct(Some(prev), ")") {
        return tokens.get(paren_open?.checked_sub(1)?);
    }
    Some(prev)
}

fn opens_fn_body(head: Option<&Token<'_>>) -> bool {
    head.is_some_and(|h| match h.kind {
        Kind::Punct => h.text == "}",
        Kind::Word => {
            h.text != "switch"
                && !BLOCK_KEYWORDS.contains(&h.text)
                && !TYPE_KEYWORDS.contains(&h.text)
        }
        Kind::Builtin => true,
        Kind::Text => false,
    })
}

fn opens_expression(head: Option<&Token<'_>>, header_fn: bool) -> bool {
    head.is_some_and(|h| match h.kind {
        Kind::Punct => h.text == ".",
        Kind::Word | Kind::Builtin => !header_fn && !BLOCK_KEYWORDS.contains(&h.text),
        Kind::Text => false,
    })
}

struct StdNames<'a> {
    std: BTreeSet<&'a str>,
    debug: BTreeSet<&'a str>,
}

impl<'a> StdNames<'a> {
    fn collect(tokens: &[Token<'a>]) -> Self {
        let mut names = StdNames {
            std: BTreeSet::from(["std"]),
            debug: BTreeSet::new(),
        };
        for want_debug in [false, true] {
            for (i, token) in tokens.iter().enumerate() {
                let (Some(name), Some(value)) = (tokens.get(i + 1), tokens.get(i + 3..)) else {
                    continue;
                };
                if !is_word(Some(token), "const")
                    || name.kind != Kind::Word
                    || !is_punct(tokens.get(i + 2), "=")
                {
                    continue;
                }
                let len = if want_debug {
                    names.debug_len(value)
                } else {
                    names.std_len(value)
                };
                if !len.is_some_and(|n| is_punct(value.get(n), ";")) {
                    continue;
                }
                if want_debug {
                    names.debug.insert(name.text);
                } else {
                    names.std.insert(name.text);
                }
            }
        }
        names
    }

    fn std_len(&self, tokens: &[Token<'_>]) -> Option<usize> {
        let first = tokens.first()?;
        if first.kind == Kind::Word && self.std.contains(first.text) {
            return Some(1);
        }
        let import = first.kind == Kind::Builtin
            && first.text == "@import"
            && is_punct(tokens.get(1), "(")
            && tokens
                .get(2)
                .is_some_and(|t| t.kind == Kind::Text && t.text == "\"std\"")
            && is_punct(tokens.get(3), ")");
        import.then_some(4)
    }

    fn debug_len(&self, tokens: &[Token<'_>]) -> Option<usize> {
        let first = tokens.first()?;
        if first.kind == Kind::Word && self.debug.contains(first.text) {
            return Some(1);
        }
        let n = self.std_len(tokens)?;
        (is_punct(tokens.get(n), ".") && is_word(tokens.get(n + 1), "debug")).then_some(n + 2)
    }

    fn is_debug_print(&self, tokens: &[Token<'_>], i: usize) -> bool {
        if i.checked_sub(1)
            .is_some_and(|p| is_punct(tokens.get(p), "."))
        {
            return false;
        }
        let rest = tokens.get(i..).unwrap_or_default();
        self.debug_len(rest)
            .is_some_and(|n| is_punct(rest.get(n), ".") && is_word(rest.get(n + 1), "print"))
    }
}

fn cast_documented(lexed: &Lexed<'_>, frames: &[Frame], start: u64, line: u64) -> bool {
    let enclosing = frames
        .iter()
        .rev()
        .take_while(|f| f.expression)
        .map(|f| f.start);
    std::iter::once(start)
        .chain(enclosing)
        .any(|s| has_safety(lexed, s, line))
}

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
    fn unreachable_catch_body_is_found() {
        assert_eq!(
            rules("f() catch |e| {\n    unreachable;\n};\n"),
            one("catch_unreachable", 1, 5)
        );
        assert_eq!(
            rules("f() catch { unreachable };\n"),
            one("catch_unreachable", 1, 5)
        );
        assert_eq!(
            rules("f() catch |e| {\n    unreachable;\n    log(e);\n};\n"),
            hits(&[])
        );
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
    fn debug_print_is_found_through_aliases_and_imports() {
        let source = "const zstd = @import(\"std\");\n\
                      const dbg = zstd.debug;\n\
                      fn f() void {\n\
                      \x20   dbg.print(\"x\", .{});\n\
                      \x20   @import(\"std\").debug.print(\"y\", .{});\n\
                      \x20   zstd.debug.print(\"z\", .{});\n\
                      \x20   dbg.assert(true);\n\
                      \x20   self.dbg.print();\n\
                      }\n\
                      test {\n\
                      \x20   dbg.print(\"ok\", .{});\n\
                      }\n";
        assert_eq!(
            rules(source),
            hits(&[
                ("debug_print", 4, 5),
                ("debug_print", 5, 5),
                ("debug_print", 6, 5),
            ])
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
        let init = "// SAFETY: n is below 256.\nconst s = S{\n    .x = @intCast(n),\n};\n";
        assert_eq!(rules(init), hits(&[]));
        let anonymous = "// SAFETY: n is below 256.\nconst s: S = .{\n    .x = @intCast(n),\n};\n";
        assert_eq!(rules(anonymous), hits(&[]));
        let switch = "// SAFETY: x is below 256.\n\
                      const v: u8 = switch (k) {\n\
                      \x20   .a => @intCast(x),\n\
                      \x20   .b => 0,\n\
                      };\n";
        assert_eq!(rules(switch), hits(&[]));
        let after_init = "// SAFETY: n is below 256.\nf(S{\n    .a = 1,\n}, @intCast(n));\n";
        assert_eq!(rules(after_init), hits(&[]));
    }

    #[test]
    fn a_safety_comment_does_not_cover_a_block() {
        let body = "// SAFETY: n fits.\nfn f(n: u32) u8 {\n    return @intCast(n);\n}\n";
        assert_eq!(rules(body), one("unsafe_cast", 3, 12));
        let generic = "// SAFETY: n fits.\nfn f(n: u32) List(u8) {\n    return @intCast(n);\n}\n";
        assert_eq!(rules(generic), one("unsafe_cast", 3, 12));
        let branch = "// SAFETY: n fits.\nif (ok) {\n    g(@intCast(n));\n}\n";
        assert_eq!(rules(branch), one("unsafe_cast", 3, 7));
        let prong = "// SAFETY: x fits.\n\
                     const v: u8 = switch (k) {\n\
                     \x20   .a => {\n\
                     \x20       return @intCast(x);\n\
                     \x20   },\n\
                     };\n";
        assert_eq!(rules(prong), one("unsafe_cast", 4, 16));
        let error_set =
            "// SAFETY: n fits.\nfn f(n: u32) error{Bad}!u8 {\n    return @intCast(n);\n}\n";
        assert_eq!(rules(error_set), one("unsafe_cast", 3, 12));
        let switch_type = "// SAFETY: n fits.\n\
                           fn f(comptime k: u1, n: u32) switch (k) { 0 => error{A}, 1 => error{B} }!u8 {\n\
                           \x20   return @intCast(n);\n\
                           }\n";
        assert_eq!(rules(switch_type), one("unsafe_cast", 3, 12));
        let block_type = "// SAFETY: n fits.\n\
                          fn f(n: u32) blk: { break :blk error{Bad}; }!u8 {\n\
                          \x20   return @intCast(n);\n\
                          }\n";
        assert_eq!(rules(block_type), one("unsafe_cast", 3, 12));
        let struct_type = "// SAFETY: n fits.\n\
                           fn f(n: u32) struct { a: u8 } {\n\
                           \x20   return .{ .a = @intCast(n) };\n\
                           }\n\
                           const s = S{\n\
                           \x20   // SAFETY: m fits.\n\
                           \x20   .a = @intCast(m),\n\
                           };\n";
        assert_eq!(rules(struct_type), one("unsafe_cast", 3, 20));
        let method = "// SAFETY: n fits.\n\
                      const S = struct {\n\
                      \x20   fn g(n: u32) u8 {\n\
                      \x20       return @intCast(n);\n\
                      \x20   }\n\
                      };\n";
        assert_eq!(rules(method), one("unsafe_cast", 4, 16));
        let after = "fn f() void {\n    // SAFETY: inside.\n}\nconst x: u8 = @intCast(n);\n";
        assert_eq!(rules(after), one("unsafe_cast", 4, 15));
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
