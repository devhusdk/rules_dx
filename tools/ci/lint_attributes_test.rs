//! No Rust source allows a strict tier lint, every `#[expect(...)]` gives a reason, the repo-wide clippy flags carry the strict tier, and no source discards a result where those lints cannot see it.

#[expect(
    dead_code,
    reason = "the guard reads only the snapshot and the Rust lexer"
)]
mod guard_scan;

use std::path::Path;
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
        .filter(|(path, _)| Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs")))
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
    assert_eq!(problems(
            "#[allow(dead_code)]\n#[expect(clippy::panic, reason = \"r\")]\nfn f() { v.expect(\"x\"); let a = [1]; }"
        ), Vec::<String>::new());
}

/// The result discards of one Rust source that `clippy::unused_result_ok` and
/// `clippy::map_err_ignore` cannot see: `drop(write!(..))`, `drop(writeln!(..))` and
/// `let _ = <expr>.ok();`.
fn discards(source: &str) -> Vec<String> {
    let tokens = rust_tokens(source);
    let word = |at: usize, text: &str| matches!(tokens.get(at), Some(Token::Word(word)) if word == text);
    let punct = |at: usize, c: char| tokens.get(at) == Some(&Token::Punct(c));
    let mut out = Vec::new();
    for at in 0..tokens.len() {
        if word(at, "drop") && punct(at + 1, '(') && punct(at + 3, '!') {
            for mac in ["write", "writeln"] {
                if word(at + 2, mac) {
                    out.push(format!("drop({mac}!(..)) discards the write error"));
                }
            }
        }
        if word(at, "let") && word(at + 1, "_") && punct(at + 2, '=') {
            let mut depth = 0usize;
            let end = (at + 3..tokens.len()).find(|&next| match tokens.get(next) {
                Some(Token::Punct('(' | '[' | '{')) => {
                    depth += 1;
                    false
                }
                Some(Token::Punct(')' | ']' | '}')) => {
                    depth = depth.saturating_sub(1);
                    false
                }
                Some(Token::Punct(';')) => depth == 0,
                Some(Token::Word(_) | Token::Text(_) | Token::Punct(_)) | None => false,
            });
            if let Some(end) = end
                && end >= at + 7
                && punct(end - 4, '.')
                && word(end - 3, "ok")
                && punct(end - 2, '(')
                && punct(end - 1, ')')
            {
                out.push("let _ = <expr>.ok() discards the error".to_owned());
            }
        }
    }
    out
}

#[test]
fn no_source_discards_a_result_unseen_by_the_lints() {
    let found: Vec<String> = snapshot()
        .iter()
        .filter(|(path, _)| Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs")))
        .flat_map(|(path, source)| {
            discards(source)
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect();
    assert!(
        found.is_empty(),
        "propagate the error, log it with context, or write diagnostics through dx_output::write_stderr:\n{}",
        found.join("\n")
    );
}

#[test]
fn flags_dropped_writes_and_ok_discards() {
    assert_eq!(
        discards("fn f() { drop(writeln!(err, \"x\")); drop(write!(out, \"{}\", a)); }"),
        [
            "drop(writeln!(..)) discards the write error",
            "drop(write!(..)) discards the write error"
        ]
    );
    assert_eq!(
        discards("fn f() { let _ = std::fs::remove_file(path(a, [b])).ok(); }"),
        ["let _ = <expr>.ok() discards the error"]
    );
}

#[test]
fn passes_handled_results_and_other_drops() {
    assert_eq!(
        discards(
            "fn f() { drop(guard); writeln!(out, \"x\")?; let _ = tx.send(1); let a = b.ok(); \
             let _ = { c.ok() }; // drop(writeln!(err))\n let s = \"drop(write!(x))\"; }"
        ),
        Vec::<String>::new()
    );
}

/// Strict tier lints `.bazelrc` does not yet deny repo-wide. The list only shrinks.
const PENDING: [&str; 0] = [];

/// The lints a file denies with `-D<lint>`, as `.bazelrc` flag lines or tier list lines.
fn denied(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("common --@rules_dx//config:clippy_flag=")
                .unwrap_or(line)
                .strip_prefix("-D")
        })
        .filter(|lint| *lint != "warnings")
        .map(str::to_owned)
        .collect()
}

#[test]
fn strict_tier_and_bazelrc_deny_the_same_rust_lints() {
    let key = std::env::var("DX_STRICT_TXT").expect("DX_STRICT_TXT is set by the test");
    let path = dx_testing::rlocation(&key).expect("strict.txt is a runfile");
    let strict = denied(&dx_testing::read_text(&path));
    for lint in [
        "unsafe_code",
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::indexing_slicing",
    ] {
        assert!(
            strict.iter().any(|denied| denied == lint),
            "the strict tier does not deny {lint}"
        );
    }
    for lint in LINTS {
        assert!(
            strict.iter().any(|denied| denied == lint),
            "LINTS names {lint}, which the strict tier does not deny"
        );
    }
    let snapshot = snapshot();
    let bazelrc = denied(
        snapshot
            .get(".bazelrc")
            .expect(".bazelrc is in the snapshot"),
    );
    let missing: Vec<&String> = strict
        .iter()
        .filter(|lint| !bazelrc.contains(lint) && !PENDING.contains(&lint.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        ".bazelrc must deny these strict tier lints: {missing:?}"
    );
    let extra: Vec<&String> = bazelrc
        .iter()
        .filter(|lint| !strict.contains(lint))
        .collect();
    assert!(
        extra.is_empty(),
        "the strict tier must deny these .bazelrc lints: {extra:?}"
    );
    let done: Vec<&str> = PENDING
        .into_iter()
        .filter(|lint| bazelrc.iter().any(|denied| denied == lint))
        .collect();
    assert!(
        done.is_empty(),
        "remove these from PENDING, .bazelrc denies them: {done:?}"
    );
}
