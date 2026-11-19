//! The shared guard helpers split source, check allowlists and match paths as the guards expect.

use dx_guard::allowlist::{
    Counts, Lines, REMOVE, add, check, check_lines, line_differences, parse, parse_lines, render,
};
use dx_guard::segments::{Piece, Segment, count_word, rust_segments, starlark_segments};
use dx_guard::snapshot::{glob_match, tree, under};
use serde_json as _;
use starlark_syntax as _;
use yaml_serde as _;

fn counts(entries: &[(&str, &str, usize)]) -> Counts {
    let mut out = Counts::new();
    for (path, kind, count) in entries {
        add(&mut out, path, kind, *count);
    }
    out
}

#[test]
fn a_spare_allowlist_line_fails() {
    let found = counts(&[("b.bzl", "alias", 2), ("c.bzl", "alias", 0)]);
    assert_eq!(
        check(&found, "a.bzl alias 1\nb.bzl alias 2\n", &["alias"], REMOVE),
        ["a.bzl alias 1: none left; delete the allowlist line"]
    );
}

#[test]
fn a_count_above_or_below_the_line_fails() {
    let found = counts(&[
        ("a.bzl", "alias", 3),
        ("b.bzl", "alias", 1),
        ("c.bzl", "alias", 1),
    ]);
    let problems = check(&found, "a.bzl alias 2\nb.bzl alias 2\n", &["alias"], REMOVE);
    assert_eq!(
        problems,
        [
            "a.bzl alias 3: 2 allowed; remove the new one",
            "c.bzl alias 1: 0 allowed; remove the new one",
            "b.bzl alias 2: 1 left; lower the allowlist count",
        ]
    );
}

#[test]
fn malformed_unsorted_and_unknown_kind_lines_fail() {
    let (_, problems) = parse(
        "b alias 1\na alias 1\na other 1\na alias 0\na alias\na alias 01\n alias 1\n",
        &["alias"],
    );
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"a alias 1\"",
            "malformed line: \"a other 1\"",
            "malformed line: \"a alias 0\"",
            "malformed line: \"a alias\"",
            "malformed line: \"a alias 01\"",
            "malformed line: \" alias 1\"",
        ]
    );
}

#[test]
fn whole_line_allowlists_report_new_stale_unsorted_and_padded_lines() {
    let found: Lines = ["a x".to_owned(), "c x".to_owned()].into();
    let fix = |line: &str| format!("drop {line}");
    assert_eq!(
        check_lines(&found, "a x\nb x\n", |line| line.ends_with('x'), fix),
        [
            "new: c x; drop c x",
            "stale: b x; delete the allowlist line"
        ]
    );
    let (lines, problems) = parse_lines("b x\na x\n c x\nd y\n", |line| line.ends_with('x'));
    assert_eq!(lines.len(), 2);
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"a x\"",
            "malformed line: \" c x\"",
            "malformed line: \"d y\""
        ]
    );
    assert!(line_differences(&found, &found, fix).is_empty());
}

#[test]
fn render_round_trips() {
    let found = counts(&[("a-b/x", "alias", 1), ("a/x", "alias", 2)]);
    let text = render(&found);
    assert_eq!(text, "a-b/x alias 1\na/x alias 2\n");
    assert_eq!(
        check(&found, &text, &["alias"], REMOVE),
        Vec::<String>::new()
    );
}

fn pieces(segments: &[Segment]) -> Vec<(Piece, &str)> {
    segments
        .iter()
        .map(|segment| (segment.piece, segment.text.as_str()))
        .collect()
}

#[test]
fn starlark_splits_comments_strings_and_docstrings() {
    let source = "\"\"\"Module doc #1.\"\"\"\nX = \"a # b\"  # note\nrule(doc = \"Attr doc.\", name = 'n')\n_T = \"\"\"code\"\"\"\n";
    let segments = starlark_segments(source);
    let found = pieces(&segments);
    assert!(found.contains(&(Piece::Doc, "Module doc #1.")), "{found:?}");
    assert!(found.contains(&(Piece::Text, "a # b")), "{found:?}");
    assert!(found.contains(&(Piece::Comment, " note")), "{found:?}");
    assert!(found.contains(&(Piece::Doc, "Attr doc.")), "{found:?}");
    assert!(found.contains(&(Piece::Text, "n")), "{found:?}");
    assert!(found.contains(&(Piece::Text, "code")), "{found:?}");
}

#[test]
fn unclosed_strings_and_comments_end_at_the_source_end() {
    assert_eq!(
        pieces(&starlark_segments("x = \"open\\")),
        [(Piece::Code, "x = "), (Piece::Text, "open\\")]
    );
    assert_eq!(
        pieces(&rust_segments("a /* open")),
        [(Piece::Code, "a "), (Piece::Comment, " open")]
    );
    assert_eq!(pieces(&rust_segments("r#\"open")), [(Piece::Text, "open")]);
}

#[test]
fn rust_splits_comments_strings_chars_and_lifetimes() {
    let source = "// top\nfn f<'a>(x: &'a str) -> char { let _ = \"a // b\\\" c\"; let _ = r#\"raw \" // x\"#; '\"' } /* block */ let q = '\\''; // end\n";
    let segments = rust_segments(source);
    let found = pieces(&segments);
    assert!(found.contains(&(Piece::Comment, " top")), "{found:?}");
    assert!(found.contains(&(Piece::Text, "a // b\\\" c")), "{found:?}");
    assert!(found.contains(&(Piece::Text, "raw \" // x")), "{found:?}");
    assert!(found.contains(&(Piece::Comment, " block ")), "{found:?}");
    assert!(found.contains(&(Piece::Comment, " end")), "{found:?}");
    assert_eq!(
        found
            .iter()
            .filter(|(piece, _)| *piece == Piece::Comment)
            .count(),
        3
    );
    assert_eq!(
        found
            .iter()
            .filter(|(piece, _)| *piece == Piece::Text)
            .count(),
        2
    );
}

#[test]
fn words_stand_alone() {
    assert_eq!(count_word("word word_x xword (word)", "word"), 2);
}

#[test]
fn globs_and_directories_match() {
    assert!(glob_match("fixtures/**", "fixtures/a/b.txt"));
    assert!(glob_match("**/*.md", "a/b/c.md"));
    assert!(glob_match("*.md", "c.md"));
    assert!(!glob_match("*.md", "a/c.md"));
    assert!(!glob_match("fixtures/*.txt", "fixtures/a/b.txt"));
    assert!(under("a/fixtures/b.md", &["fixtures"]));
    assert!(!under("a/fixtures", &["fixtures"]));
    assert!(!under("fixtures", &["fixtures"]));
}

#[test]
fn the_tree_lists_files_by_relative_path() {
    let guard = dx_test_scratch::scratch("tree").unwrap();
    let root = guard.path();
    dx_testing::write_text(&root.join("a/b.txt"), "b\n");
    dx_testing::write_text(&root.join("c.txt"), "c\n");
    assert_eq!(
        tree(root).unwrap(),
        [
            ("a/b.txt".to_owned(), "b\n".to_owned()),
            ("c.txt".to_owned(), "c\n".to_owned()),
        ]
    );
}
