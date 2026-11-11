//! What the source guards share: the snapshot, a Starlark walk, a Rust lexer and the allowlist check.

use std::collections::BTreeMap;

use starlark_syntax::syntax::ast::{AstExpr, AstLiteral, AstStmt, ExprP};
use starlark_syntax::syntax::{AstModule, Dialect};

/// The guard sources, which hold the patterns they detect.
const GUARDS: [&str; 4] = [
    "tools/ci/guard_scan.rs",
    "tools/ci/host_env_test.rs",
    "tools/ci/host_program_test.rs",
    "tools/ci/sandbox_escape_test.rs",
];

/// Endings of Starlark file names.
const STARLARK: [&str; 3] = ["BUILD.bazel", ".BUILD", ".bzl"];

/// Endings of the other scanned files.
const CODE: [&str; 7] = [".rs", ".sh", ".tpl", ".bazelrc", ".bat", ".cmd", ".ps1"];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 2000;

/// Occurrences keyed by file path and kind.
pub type Counts = BTreeMap<(String, String), usize>;

/// Adds one occurrence of `kind` in `path`.
pub fn hit(counts: &mut Counts, path: &str, kind: &str) {
    *counts
        .entry((path.to_owned(), kind.to_owned()))
        .or_default() += 1;
}

/// One token of Rust source. Comments and character literals are dropped.
#[derive(Debug, PartialEq)]
pub enum Token {
    Word(String),
    Text(String),
    Punct(char),
}

/// Reads one runfile named by an environment variable.
fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

/// The snapshot as a map from path to text, without the guard sources.
pub fn snapshot() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for variable in ["DX_SOURCES_JSON", "DX_FILES_JSON", "DX_CODE_JSON"] {
        let part: BTreeMap<String, String> = serde_json::from_str(&read_input(variable))
            .unwrap_or_else(|error| panic!("{variable}: {error}"));
        files.extend(part);
    }
    assert!(
        files.len() >= MIN_FILES,
        "only {} files in the snapshot",
        files.len()
    );
    files.retain(|path, _| {
        let scanned = is_starlark(path) || CODE.iter().any(|end| path.ends_with(end));
        scanned && !GUARDS.contains(&path.as_str())
    });
    files
}

/// Whether `path` names a BUILD or .bzl file, or the BUILD file of a repository.
pub fn is_starlark(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "BUILD" || STARLARK.iter().any(|end| name.ends_with(end))
}

/// Parses one BUILD or .bzl file.
pub fn parse(path: &str, source: &str) -> AstModule {
    AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()))
}

fn collect<'a>(expr: &'a AstExpr, out: &mut Vec<&'a AstExpr>) {
    out.push(expr);
    expr.visit_expr(|child| collect(child, out));
}

/// Every expression under `stmt`, outer before inner.
pub fn exprs(stmt: &AstStmt) -> Vec<&AstExpr> {
    let mut out = Vec::new();
    stmt.visit_expr(|expr| collect(expr, &mut out));
    out
}

/// The last name of a callee: `native.genrule` gives `genrule`.
pub fn callee_name(callee: &AstExpr) -> &str {
    match &callee.node {
        ExprP::Identifier(ident) => &ident.node.ident,
        ExprP::Dot(_, name) => &name.node,
        _ => "",
    }
}

/// The text of a string literal.
pub fn string(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(&text.node),
        _ => None,
    }
}

/// Lexes Rust source into words, string literals and punctuation.
pub fn rust_tokens(source: &str) -> Vec<Token> {
    Lexer {
        chars: source.chars().collect(),
        at: 0,
        out: Vec::new(),
    }
    .run()
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    out: Vec<Token>,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn run(mut self) -> Vec<Token> {
        while let Some(current) = self.peek(0) {
            if current.is_whitespace() {
                self.at += 1;
            } else if current == '/' && self.peek(1) == Some('/') {
                while self.peek(0).is_some_and(|next| next != '\n') {
                    self.at += 1;
                }
            } else if current == '/' && self.peek(1) == Some('*') {
                self.block_comment();
            } else if current == '"' {
                self.at += 1;
                self.quoted();
            } else if current == '\'' {
                self.quote();
            } else if current.is_alphanumeric() || current == '_' {
                self.word();
            } else {
                self.out.push(Token::Punct(current));
                self.at += 1;
            }
        }
        self.out
    }

    fn block_comment(&mut self) {
        let mut depth = 0usize;
        while let Some(current) = self.peek(0) {
            if current == '/' && self.peek(1) == Some('*') {
                depth += 1;
                self.at += 2;
            } else if current == '*' && self.peek(1) == Some('/') {
                depth -= 1;
                self.at += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.at += 1;
            }
        }
    }

    /// Reads a string body after its opening quote.
    fn quoted(&mut self) {
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            self.at += 1;
            match current {
                '"' => break,
                '\\' => {
                    let escaped = self.peek(0).unwrap_or('\\');
                    self.at += 1;
                    match escaped {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        '\n' => {
                            while self.peek(0).is_some_and(char::is_whitespace) {
                                self.at += 1;
                            }
                        }
                        other => text.push(other),
                    }
                }
                other => text.push(other),
            }
        }
        self.out.push(Token::Text(text));
    }

    /// Reads a raw string body after its opening quote, closed by a quote and `hashes` hashes.
    fn raw(&mut self, hashes: usize) {
        let mut text = String::new();
        while let Some(current) = self.peek(0) {
            self.at += 1;
            if current == '"' && (0..hashes).all(|index| self.peek(index) == Some('#')) {
                self.at += hashes;
                break;
            }
            text.push(current);
        }
        self.out.push(Token::Text(text));
    }

    /// Skips a character literal, or the quote of a lifetime.
    fn quote(&mut self) {
        if self.peek(1) == Some('\\') {
            self.at += 3;
            while self.peek(0).is_some_and(|next| next != '\'') {
                self.at += 1;
            }
            self.at += 1;
        } else if self.peek(2) == Some('\'') {
            self.at += 3;
        } else {
            self.at += 1;
        }
    }

    fn word(&mut self) {
        let start = self.at;
        while self
            .peek(0)
            .is_some_and(|next| next.is_alphanumeric() || next == '_')
        {
            self.at += 1;
        }
        let word: String = self.chars[start..self.at].iter().collect();
        let prefix = matches!(word.as_str(), "b" | "c" | "r" | "br" | "cr");
        let mut hashes = 0;
        while word.ends_with('r') && self.peek(hashes) == Some('#') {
            hashes += 1;
        }
        if prefix && self.peek(hashes) == Some('"') {
            self.at += hashes + 1;
            if word.ends_with('r') {
                self.raw(hashes);
            } else {
                self.quoted();
            }
        } else if word == "b" && self.peek(0) == Some('\'') {
            self.quote();
        } else {
            self.out.push(Token::Word(word));
        }
    }
}

/// One allowlist line: `<path> <kind> <count>`.
fn parse_allowlist(text: &str, kinds: &[&str]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, kind, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count.parse::<usize>().unwrap_or(0);
        if count == 0 || !kinds.contains(kind) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        let key = ((*path).to_owned(), (*kind).to_owned());
        if previous.as_ref().is_some_and(|before| *before >= key) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(key.clone());
        counts.insert(key, count);
    }
    (counts, problems)
}

/// Lines that differ between the tree and the allowlist text.
pub fn problems(found: &Counts, allowlist: &str, kinds: &[&str]) -> Vec<String> {
    let (allowed, mut out) = parse_allowlist(allowlist, kinds);
    for (key, count) in found {
        let limit = allowed.get(key).copied().unwrap_or(0);
        if *count > limit {
            out.push(format!(
                "{} {} {count}: {limit} allowed; remove the new one",
                key.0, key.1
            ));
        }
    }
    for (key, limit) in &allowed {
        let count = found.get(key).copied().unwrap_or(0);
        if count == 0 {
            out.push(format!(
                "{} {} {limit}: none left; delete the allowlist line",
                key.0, key.1
            ));
        } else if count < *limit {
            out.push(format!(
                "{} {} {limit}: {count} left; lower the allowlist count",
                key.0, key.1
            ));
        }
    }
    out
}

/// Fails when `found` and the allowlist named by `ALLOWLIST` disagree.
pub fn verify(found: &Counts, kinds: &[&str]) {
    let key = std::env::var("ALLOWLIST").unwrap_or_else(|_| panic!("ALLOWLIST is set by the test"));
    let problems = problems(found, &read_input("ALLOWLIST"), kinds);
    assert!(
        problems.is_empty(),
        "{key} and the tree disagree:\n{}",
        problems.join("\n")
    );
}
