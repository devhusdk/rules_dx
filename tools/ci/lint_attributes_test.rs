//! No Rust source allows a strict tier lint, and every `#[expect(...)]` gives a reason.

#[expect(
    dead_code,
    reason = "the guard reads only the snapshot and the Rust lexer"
)]
mod guard_scan;

use guard_scan::{Token, rust_tokens, snapshot};
use yaml_serde as _;

/// The lints of core/quality/tiers/clippy/strict.txt that no source may allow.
const LINTS: [&str; 21] = [
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::exit",
    "clippy::expect_used",
    "clippy::fallible_impl_from",
    "clippy::get_unwrap",
    "clippy::indexing_slicing",
    "clippy::let_underscore_must_use",
    "clippy::panic",
    "clippy::panic_in_result_fn",
    "clippy::string_slice",
    "clippy::todo",
    "clippy::undocumented_unsafe_blocks",
    "clippy::unimplemented",
    "clippy::unreachable",
    "clippy::unwrap_in_result",
    "clippy::unwrap_used",
    "clippy::wildcard_enum_match_arm",
    "unsafe_code",
    "unsafe_op_in_unsafe_fn",
    "unused_must_use",
];

/// The lint paths named inside one parenthesised attribute argument list.
fn lint_paths(args: &[Token]) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(token) = args.get(at) {
        if let Token::Word(word) = token {
            let mut path = word.clone();
            while let (Some(Token::Punct(':')), Some(Token::Punct(':')), Some(Token::Word(next))) =
                (args.get(at + 1), args.get(at + 2), args.get(at + 3))
            {
                path = format!("{path}::{next}");
                at += 3;
            }
            out.push(path);
        }
        at += 1;
    }
    out
}

/// The tokens between the parenthesis at `open` and its match.
fn group(tokens: &[Token], open: usize) -> &[Token] {
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate().skip(open) {
        match token {
            Token::Punct('(') => depth += 1,
            Token::Punct(')') => {
                depth -= 1;
                if depth == 0 {
                    return tokens.get(open + 1..at).unwrap_or_default();
                }
            }
            Token::Word(_) | Token::Text(_) | Token::Punct(_) => {}
        }
    }
    tokens.get(open + 1..).unwrap_or_default()
}

/// The problems of one Rust source.
fn problems(source: &str) -> Vec<String> {
    let tokens = rust_tokens(source);
    let mut out = Vec::new();
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        match token {
            Token::Punct('[') if depth > 0 => depth += 1,
            Token::Punct('[') => {
                let hash = at.checked_sub(1).and_then(|before| tokens.get(before));
                let bang = at.checked_sub(2).and_then(|before| tokens.get(before));
                if hash == Some(&Token::Punct('#'))
                    || (hash == Some(&Token::Punct('!')) && bang == Some(&Token::Punct('#')))
                {
                    depth = 1;
                }
            }
            Token::Punct(']') if depth > 0 => depth -= 1,
            Token::Word(word) if depth > 0 && tokens.get(at + 1) == Some(&Token::Punct('(')) => {
                let args = group(&tokens, at + 1);
                if word == "allow" {
                    out.extend(
                        lint_paths(args)
                            .into_iter()
                            .filter(|path| LINTS.contains(&path.as_str()))
                            .map(|path| format!("allows {path}")),
                    );
                } else if word == "expect" && !args.contains(&Token::Word("reason".to_owned())) {
                    out.push("expects a lint without a reason".to_owned());
                }
            }
            Token::Word(_) | Token::Text(_) | Token::Punct(_) => {}
        }
    }
    out
}

#[test]
fn no_source_allows_a_strict_lint_or_expects_without_reason() {
    let found: Vec<String> = snapshot()
        .iter()
        .filter(|(path, _)| path.ends_with(".rs"))
        .flat_map(|(path, source)| {
            problems(source)
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect();
    assert!(
        found.is_empty(),
        "use #[expect(<lint>, reason = \"...\")] instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn flags_allow_of_a_strict_lint() {
    assert_eq!(
        problems("#[allow(clippy::unwrap_used)]\nfn main() {}"),
        ["allows clippy::unwrap_used"]
    );
    assert_eq!(
        problems("#![cfg_attr(test, allow(dead_code, unsafe_code))]"),
        ["allows unsafe_code"]
    );
}

#[test]
fn flags_expect_without_reason() {
    assert_eq!(
        problems("#[expect(clippy::panic)]\nfn main() {}"),
        ["expects a lint without a reason"]
    );
}

#[test]
fn passes_other_attributes() {
    assert!(
        problems(
            "#[allow(dead_code)]\n#[expect(clippy::panic, reason = \"r\")]\nfn f() { v.expect(\"x\"); let a = [1]; }"
        )
        .is_empty()
    );
}
