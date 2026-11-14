//! No Rust code or repository rule starts a host program by name outside tools/ci/host_program_allowlist.txt, and the allowlist holds no spare entry.

mod guard_scan;

use guard_scan::{Counts, Token, callee_name, exprs, hit, is_starlark, parse, rust_tokens, string};
use starlark_syntax::syntax::ast::{ArgumentP, ExprP};
use yaml_serde as _;

/// The kinds an allowlist line may name.
const KINDS: [&str; 5] = [
    "env-home",
    "env-path",
    "rctx-execute-by-name",
    "rctx-which",
    "spawn-by-name",
];

/// The `std::env` readers of one variable.
const READERS: [&str; 2] = ["var", "var_os"];

/// Whether `program` names a program without a path.
fn bare_name(program: &str) -> bool {
    !program.is_empty() && !program.contains(['/', '\\'])
}

/// The kind of reading the host variable `name`.
fn env_kind(name: &str) -> Option<&'static str> {
    match name {
        "PATH" => Some("env-path"),
        "HOME" | "USERPROFILE" => Some("env-home"),
        _ => None,
    }
}

fn is_punct(token: Option<&Token>, wanted: char) -> bool {
    matches!(token, Some(Token::Punct(found)) if *found == wanted)
}

fn word(token: Option<&Token>) -> &str {
    match token {
        Some(Token::Word(found)) => found,
        _ => "",
    }
}

fn scan_rust(path: &str, source: &str, counts: &mut Counts) {
    let tokens = rust_tokens(source);
    for at in 0..tokens.len() {
        let called = is_punct(tokens.get(at + 1), ':')
            && is_punct(tokens.get(at + 2), ':')
            && is_punct(tokens.get(at + 4), '(');
        let Some(Token::Text(literal)) = tokens.get(at + 5) else {
            continue;
        };
        if !called {
            continue;
        }
        let owner = word(tokens.get(at));
        let function = word(tokens.get(at + 3));
        if owner == "Command" && function == "new" && bare_name(literal) {
            hit(counts, path, "spawn-by-name");
        }
        if owner == "env"
            && READERS.contains(&function)
            && let Some(kind) = env_kind(literal)
        {
            hit(counts, path, kind);
        }
    }
}

fn scan_starlark(path: &str, source: &str, counts: &mut Counts) {
    let module = parse(path, source);
    for expr in exprs(module.statement()) {
        let ExprP::Call(callee, args) = &expr.node else {
            continue;
        };
        if !matches!(&callee.node, ExprP::Dot(..)) {
            continue;
        }
        match callee_name(callee) {
            "which" => hit(counts, path, "rctx-which"),
            "execute" => {
                let first = args.args.first().and_then(|arg| match &arg.node {
                    ArgumentP::Positional(value) => Some(value),
                    ArgumentP::Named(key, value) if key.node == "arguments" => Some(value),
                    ArgumentP::Named(..) | ArgumentP::Args(_) | ArgumentP::KwArgs(_) => None,
                });
                let program = first.and_then(|value| match &value.node {
                    ExprP::List(items) => items.first().and_then(string),
                    ExprP::Tuple(_)
                    | ExprP::Dot(..)
                    | ExprP::Call(..)
                    | ExprP::Index(_)
                    | ExprP::Index2(_)
                    | ExprP::Slice(..)
                    | ExprP::Identifier(_)
                    | ExprP::Lambda(_)
                    | ExprP::Literal(_)
                    | ExprP::Not(_)
                    | ExprP::Minus(_)
                    | ExprP::Plus(_)
                    | ExprP::BitNot(_)
                    | ExprP::Op(..)
                    | ExprP::If(_)
                    | ExprP::Dict(_)
                    | ExprP::ListComprehension(..)
                    | ExprP::DictComprehension(..)
                    | ExprP::FString(_) => None,
                });
                if program.is_some_and(bare_name) {
                    hit(counts, path, "rctx-execute-by-name");
                }
            }
            _ => {}
        }
    }
}

/// Host programs one file starts or looks up by name.
fn scan(path: &str, source: &str, counts: &mut Counts) {
    if is_starlark(path) {
        scan_starlark(path, source, counts);
    } else if path.ends_with(".rs") {
        scan_rust(path, source, counts);
    }
}

fn scan_all(sources: &[(&str, &str)]) -> Vec<(String, String, usize)> {
    let mut counts = Counts::new();
    for (path, source) in sources {
        scan(path, source, &mut counts);
    }
    counts
        .into_iter()
        .map(|((path, kind), count)| (path, kind, count))
        .collect()
}

fn lines(entries: &[(&str, &str, usize)]) -> Vec<(String, String, usize)> {
    entries
        .iter()
        .map(|(path, kind, count)| ((*path).to_owned(), (*kind).to_owned(), *count))
        .collect()
}

fn text(value: &str) -> Token {
    Token::Text(value.to_owned())
}

fn name(value: &str) -> Token {
    Token::Word(value.to_owned())
}

#[test]
fn the_tree_matches_the_allowlist() {
    let mut counts = Counts::new();
    for (path, source) in &guard_scan::snapshot() {
        scan(path, source, &mut counts);
    }
    guard_scan::verify(&counts, &KINDS);
}

#[test]
fn the_lexer_drops_comments_and_keeps_string_text() {
    let source = r###"
// "line"
/* "block" /* "nested" */ */
let a = "plain \"quoted\"\n";
let b = r#"raw "inner""#;
let c = b"bytes";
let d: &'static str = 'x'.to_string() + '\''.to_string();
"###;
    let tokens: Vec<Token> = rust_tokens(source)
        .into_iter()
        .filter(|token| !matches!(token, Token::Punct(_)))
        .collect();
    assert_eq!(
        tokens,
        [
            name("let"),
            name("a"),
            text("plain \"quoted\"\n"),
            name("let"),
            name("b"),
            text("raw \"inner\""),
            name("let"),
            name("c"),
            text("bytes"),
            name("let"),
            name("d"),
            name("static"),
            name("str"),
            name("to_string"),
            name("to_string"),
        ],
    );
}

#[test]
fn a_program_started_by_name_counts() {
    let rust = r##"
use std::process::Command;
fn run(tool: &std::path::Path) {
    Command::new("git").status();
    std::process::Command::new("tar").status();
    tokio::process::Command::new("cmd.exe");
    Command::new("/bin/sh").status();
    Command::new("bin\\tool.exe").status();
    Command::new(tool).status();
    // Command::new("grep")
    let text = r#"Command::new("sed")"#;
}
#[cfg(test)]
mod tests {
    #[test]
    fn spawns() {
        std::process::Command::new("bazel").status();
    }
}
"##;
    assert_eq!(
        scan_all(&[
            ("cli/tool/src/lib.rs", rust),
            (
                "deploy/private/src/push.rs",
                "fn tag() { Command::new(\"git\"); }\n"
            ),
        ]),
        lines(&[
            ("cli/tool/src/lib.rs", "spawn-by-name", 4),
            ("deploy/private/src/push.rs", "spawn-by-name", 1),
        ]),
    );
}

#[test]
fn reads_of_the_host_path_and_home_count() {
    let rust = r#"
fn dirs() {
    let path = std::env::var("PATH");
    let path = env::var_os("PATH");
    let home = std::env::var("HOME");
    let profile = std::env::var_os("USERPROFILE");
    let tmp = std::env::var("TEST_TMPDIR");
    command.env("PATH", "/dx_no_path");
    std::env::var(name);
}
"#;
    assert_eq!(
        scan_all(&[("cli/tool/src/lib.rs", rust)]),
        lines(&[
            ("cli/tool/src/lib.rs", "env-home", 2),
            ("cli/tool/src/lib.rs", "env-path", 2),
        ]),
    );
}

#[test]
fn repository_rules_that_look_up_or_run_a_host_program_count() {
    let bzl = r#"
def _impl(rctx):
    python = rctx.which("python3")
    xcode = repository_ctx.which("xcodebuild")
    rctx.execute(["tar", "-xf", archive])
    module_ctx.execute(arguments = ["git", "status"])
    rctx.execute([python, "-c", "pass"])
    rctx.execute(["bin/tool", "run"])
    rctx.execute([str(rctx.path("tool"))])
    which("python3")
"#;
    assert_eq!(
        scan_all(&[("pkg/repos.bzl", bzl)]),
        lines(&[
            ("pkg/repos.bzl", "rctx-execute-by-name", 2),
            ("pkg/repos.bzl", "rctx-which", 2),
        ]),
    );
}

#[test]
fn a_spare_or_loose_allowlist_line_fails() {
    let mut found = Counts::new();
    hit(&mut found, "cli/process/src/capture.rs", "spawn-by-name");
    let check = |allowlist: &str| guard_scan::problems(&found, allowlist, &KINDS);
    assert_eq!(
        check("cli/process/src/capture.rs spawn-by-name 1\n"),
        Vec::<String>::new(),
    );
    assert_eq!(
        check(""),
        ["cli/process/src/capture.rs spawn-by-name 1: 0 allowed; remove the new one"],
    );
    assert_eq!(
        check("cli/process/src/capture.rs spawn-by-name 2\n"),
        ["cli/process/src/capture.rs spawn-by-name 2: 1 left; lower the allowlist count"],
    );
    assert_eq!(
        check(
            "cli/process/src/capture.rs spawn-by-name 1\ncli/test_scratch/src/lib.rs spawn-by-name 1\n"
        ),
        ["cli/test_scratch/src/lib.rs spawn-by-name 1: none left; delete the allowlist line"],
    );
}
