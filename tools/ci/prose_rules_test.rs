//! Docs and comments outside fixtures, testdata and third_party carry no issue number, link chain or history word, design pages stay under the word limit, and every page is linted and links to tracked files, outside tools/ci/prose_rules_allowlist.txt.

use std::collections::{BTreeMap, BTreeSet};

use serde_json as _;
use shared::{Counts, Piece, add, count_word, glob_match, is_starlark, under};
use starlark_syntax as _;
use yaml_serde as _;

#[path = "guards/shared.rs"]
mod shared;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/prose_rules_allowlist.txt";

/// The kinds an allowlist line may name.
const KINDS: [&str; 5] = [
    "dead-link",
    "history-word",
    "issue-number",
    "link-chain",
    "not-linted",
];

/// The kind no allowlist line may name.
const WORD_LIMIT_KIND: &str = "word-limit";

/// A design page with this many words is too long.
const WORD_LIMIT: usize = 1300;

/// The design pages over the limit, each with the word count it may not pass.
const OVER_LIMIT: [(&str, usize); 1] = [("docs/design/tests.md", 1611)];

/// Words that tell history instead of a rule.
const HISTORY_WORDS: [&str; 7] = [
    "previously",
    "used to",
    "no longer",
    "formerly",
    "historically",
    "rationale",
    "we decided",
];

/// Markers that chain a comment to another document.
const CHAIN_MARKERS: [&str; 3] = ["See:", "Contract:", "policy:"];

/// Directory names whose files hold test input or vendored text.
const SKIPPED_DIRS: [&str; 3] = ["fixtures", "testdata", "third_party"];

/// The `README.md` section that may tell history.
const REPLACES_SECTION: &str = "What rules_dx replaces";

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 3000;

/// How often `#` and 3 to 5 digits, or an issue or pull request URL, stands in `text`.
fn issue_numbers(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    for (start, _) in text.match_indices('#') {
        let digits = bytes[start + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let after = bytes.get(start + 1 + digits);
        let glued = start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'&');
        if (3..=5).contains(&digits) && !glued && !after.is_some_and(u8::is_ascii_alphanumeric) {
            count += 1;
        }
    }
    for marker in ["/issues/", "/pull/"] {
        count += text
            .match_indices(marker)
            .filter(|(start, _)| bytes.get(start + marker.len()).is_some_and(u8::is_ascii_digit))
            .count();
    }
    count
}

/// How often a chain marker or `ADR` stands in `text`.
fn link_chains(text: &str) -> usize {
    let bytes = text.as_bytes();
    let markers: usize = CHAIN_MARKERS
        .iter()
        .map(|marker| {
            text.match_indices(marker)
                .filter(|(start, _)| *start == 0 || !bytes[start - 1].is_ascii_alphanumeric())
                .count()
        })
        .sum();
    markers + count_word(text, "ADR")
}

/// The comments and docstrings of one source file, without `LCOV_EXCL` lines.
fn comment_lines(path: &str, source: &str) -> Vec<String> {
    let segments = if is_starlark(path) {
        shared::starlark_segments(source)
    } else {
        shared::rust_segments(source)
    };
    segments
        .iter()
        .filter(|segment| matches!(segment.piece, Piece::Comment | Piece::Doc))
        .flat_map(|segment| segment.text.lines())
        .filter(|line| !line.contains("LCOV_EXCL"))
        .map(str::to_owned)
        .collect()
}

/// `text` without the section titled `title`.
fn without_section(text: &str, title: &str) -> String {
    let mut out = String::new();
    let mut skipping: Option<usize> = None;
    for line in text.lines() {
        let level = line.bytes().take_while(|byte| *byte == b'#').count();
        if level > 0 && skipping.is_some_and(|open| level <= open) {
            skipping = None;
        }
        if level > 0 && line.contains(title) {
            skipping = Some(level);
        }
        if skipping.is_none() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// How often a history word stands in `text`.
fn history_words(text: &str) -> usize {
    let lower = text.to_lowercase();
    HISTORY_WORDS
        .iter()
        .map(|word| count_word(&lower, word))
        .sum()
}

/// The directory of `path`, `""` at the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{dir}/{rel}")
    }
}

/// The string literals of each `attr = ...` value in `source`, with whether a `glob(` holds them.
fn attr_strings(source: &str, attr: &str) -> Vec<(String, bool)> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    for (found, _) in source.match_indices(attr) {
        let before_ok = found == 0 || !(bytes[found - 1].is_ascii_alphanumeric() || bytes[found - 1] == b'_');
        let rest = source[found + attr.len()..].trim_start();
        if !before_ok || !rest.starts_with('=') || rest.starts_with("==") {
            continue;
        }
        let value = &rest[1..];
        let value_bytes = value.as_bytes();
        let (mut index, mut depth, mut glob_depth) = (0, 0_i32, None);
        while index < value_bytes.len() {
            match value_bytes[index] {
                b'(' | b'[' | b'{' => {
                    if value_bytes[index] == b'(' && value[..index].trim_end().ends_with("glob") {
                        glob_depth.get_or_insert(depth);
                    }
                    depth += 1;
                }
                b')' | b']' | b'}' => {
                    depth -= 1;
                    if glob_depth == Some(depth) {
                        glob_depth = None;
                    }
                    if depth < 0 {
                        break;
                    }
                }
                b',' if depth == 0 => break,
                b'"' => {
                    let end = value[index + 1..].find('"').map_or(value.len(), |offset| index + 1 + offset);
                    out.push((value[index + 1..end].to_owned(), glob_depth.is_some()));
                    index = end;
                }
                _ => {}
            }
            index += 1;
        }
    }
    out
}

/// Every Markdown path a `markdown_srcs` or `markdown_siblings` value in a BUILD file names.
fn linted(files: &BTreeMap<String, String>) -> BTreeSet<String> {
    let pages: Vec<&String> = files.keys().filter(|path| path.ends_with(".md")).collect();
    let mut out = BTreeSet::new();
    for (path, source) in files {
        if !matches!(path.rsplit('/').next(), Some("BUILD" | "BUILD.bazel")) {
            continue;
        }
        let package = parent(path);
        for attr in ["markdown_srcs", "markdown_siblings"] {
            for (text, glob) in attr_strings(source, attr) {
                let named = if let Some(label) = text.strip_prefix("//") {
                    label.split_once(':').map(|(dir, name)| join(dir, name))
                } else if text.starts_with('@') {
                    None
                } else {
                    Some(join(package, text.trim_start_matches(':')))
                };
                let Some(named) = named else { continue };
                if glob {
                    out.extend(pages.iter().filter(|page| glob_match(&named, page)).map(|page| (*page).clone()));
                } else {
                    out.insert(named);
                }
            }
        }
    }
    out
}

/// The relative link targets of one Markdown text, outside fenced code.
fn link_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for (start, _) in line.match_indices("](") {
            let rest = &line[start + 2..];
            let end = rest.find([')', ' ']).unwrap_or(rest.len());
            let target = rest[..end].trim_matches(['<', '>']);
            let file = target.split(['#', '?']).next().unwrap_or("");
            if !file.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
                out.push(file.to_owned());
            }
        }
    }
    out
}

/// The path `target` names from the page at `page`, or `None` when it leaves the tree.
fn resolve(page: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        parent(page).split('/').filter(|part| !part.is_empty()).collect()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// Counts every hit of every kind in `files`; `paths` lists every tracked path.
fn scan(files: &BTreeMap<String, String>, paths: &[String]) -> Counts {
    let mut counts = Counts::new();
    let mut known: BTreeSet<&str> = BTreeSet::new();
    for path in paths {
        known.insert(path);
        let mut dir = parent(path);
        while !dir.is_empty() && known.insert(dir) {
            dir = parent(dir);
        }
    }
    let linted = linted(files);
    for (path, text) in files {
        if under(path, &SKIPPED_DIRS) {
            continue;
        }
        if path.ends_with(".md") {
            add(&mut counts, path, "issue-number", issue_numbers(text));
            let told = if path == "README.md" {
                without_section(text, REPLACES_SECTION)
            } else {
                text.clone()
            };
            add(&mut counts, path, "history-word", history_words(&told));
            let design = path.strip_prefix("docs/design/").is_some_and(|name| !name.contains('/'));
            let words = text.split_whitespace().count();
            if design && words >= WORD_LIMIT {
                add(&mut counts, path, WORD_LIMIT_KIND, words);
            }
            add(&mut counts, path, "not-linted", usize::from(!linted.contains(path)));
            let dead = link_targets(text)
                .iter()
                .filter(|target| resolve(path, target).is_none_or(|file| !file.is_empty() && !known.contains(file.as_str())))
                .count();
            add(&mut counts, path, "dead-link", dead);
        } else if is_starlark(path) || path.ends_with(".rs") {
            let lines = comment_lines(path, text);
            add(&mut counts, path, "issue-number", lines.iter().map(|line| issue_numbers(line)).sum());
            add(&mut counts, path, "link-chain", lines.iter().map(|line| link_chains(line)).sum());
        }
    }
    counts
}

/// Every way the word-limit hits in `found` and `caps` disagree.
fn long_pages(found: &Counts, caps: &[(&str, usize)]) -> Vec<String> {
    let mut problems = Vec::new();
    let hits: BTreeMap<&str, usize> = found
        .iter()
        .filter(|((_, kind), _)| kind == WORD_LIMIT_KIND)
        .map(|((path, _), words)| (path.as_str(), *words))
        .collect();
    for (path, words) in &hits {
        match caps.iter().find(|(capped, _)| capped == path) {
            None => problems.push(format!("{path}: {words} words; cut it under {WORD_LIMIT}")),
            Some((_, cap)) if words > cap => problems.push(format!("{path}: {words} words, capped at {cap}; cut it")),
            Some(_) => {}
        }
    }
    for (path, _) in caps {
        if !hits.contains_key(path) {
            problems.push(format!("{path}: under {WORD_LIMIT} words; delete its OVER_LIMIT entry"));
        }
    }
    problems
}

#[test]
fn prose_rules_hold_outside_the_allowlist() {
    let files = shared::tree(&dx_testing::source_snapshot());
    assert!(files.len() >= MIN_FILES, "only {} files in the snapshot", files.len());
    let mut found = scan(&files, &dx_testing::source_paths());
    let long = long_pages(&found, &OVER_LIMIT);
    assert!(long.is_empty(), "design pages and the {WORD_LIMIT} word limit disagree:\n{}", long.join("\n"));
    found.retain(|(_, kind), _| kind != WORD_LIMIT_KIND);
    shared::settle(&found, "DX_ALLOWLIST", ALLOWLIST, &KINDS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> (BTreeMap<String, String>, Vec<String>) {
        let files: BTreeMap<String, String> = entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        let paths = files.keys().cloned().collect();
        (files, paths)
    }

    fn hits(entries: &[(&str, &str)]) -> Vec<(String, String, usize)> {
        let (files, paths) = tree(entries);
        scan(&files, &paths)
            .into_iter()
            .map(|((path, kind), count)| (path, kind, count))
            .collect()
    }

    fn hit(path: &str, kind: &str, count: usize) -> (String, String, usize) {
        (path.to_owned(), kind.to_owned(), count)
    }

    const LINTED: (&str, &str) = ("BUILD.bazel", "t(markdown_srcs = glob([\"**/*.md\"]))\n");

    #[test]
    fn issue_numbers_are_detected_in_pages_comments_and_docstrings() {
        assert_eq!(issue_numbers("fixed (#1234), see #12, #123456, a#123, &#123; and #999."), 2);
        assert_eq!(issue_numbers("https://github.com/o/r/issues/12 and /pull/3 and /pull/x"), 2);
        let found = hits(&[
            LINTED,
            ("a.md", "Fixed in #1234.\n"),
            ("a.bzl", "\"\"\"Doc (#1234).\"\"\"\nX = \"#1234\"  # floor (#2283, #3736)\n"),
            ("a.rs", "// see #4321\nconst X: &str = \"#1234\"; // LCOV_EXCL_LINE reason: x issue: #1234\n"),
            ("fixtures/b.md", "Fixed in #1234.\n"),
        ]);
        assert_eq!(
            found,
            [
                hit("a.bzl", "issue-number", 3),
                hit("a.md", "issue-number", 1),
                hit("a.rs", "issue-number", 1),
            ]
        );
    }

    #[test]
    fn link_chains_are_detected_in_comments_and_docstrings_only() {
        let found = hits(&[
            ("a.bzl", "\"\"\"Doc. See: x. ADR 4.\"\"\"\nX = \"See: y\"  # Contract: z, policy: q\n"),
            ("a.rs", "/// See: the page.\nconst X: &str = \"ADR\"; // BADR, ADRs\n"),
        ]);
        assert_eq!(found, [hit("a.bzl", "link-chain", 4), hit("a.rs", "link-chain", 1)]);
    }

    #[test]
    fn history_words_are_detected_outside_the_replaces_section() {
        let readme = "# T\nIt Used To work.\n## What rules_dx replaces\nPreviously x.\n### More\nformerly\n## Next\nNo longer.\n";
        let found = hits(&[LINTED, ("README.md", readme), ("docs/a.md", "The rationale: we decided, historically.\n")]);
        assert_eq!(
            found,
            [hit("README.md", "history-word", 2), hit("docs/a.md", "history-word", 3)]
        );
    }

    #[test]
    fn a_design_page_at_the_word_limit_is_detected() {
        let at = "word ".repeat(WORD_LIMIT);
        let under = "word ".repeat(WORD_LIMIT - 1);
        let found = hits(&[
            LINTED,
            ("docs/design/a.md", at.as_str()),
            ("docs/design/b.md", under.as_str()),
            ("docs/c.md", at.as_str()),
        ]);
        assert_eq!(found, [hit("docs/design/a.md", WORD_LIMIT_KIND, WORD_LIMIT)]);
        let mut counts = Counts::new();
        add(&mut counts, "docs/design/a.md", WORD_LIMIT_KIND, WORD_LIMIT + 5);
        assert_eq!(long_pages(&counts, &[]).len(), 1);
        assert!(long_pages(&counts, &[("docs/design/a.md", WORD_LIMIT + 5)]).is_empty());
        assert_eq!(long_pages(&counts, &[("docs/design/a.md", WORD_LIMIT + 4)]).len(), 1);
        assert_eq!(long_pages(&Counts::new(), &[("docs/design/a.md", WORD_LIMIT)]).len(), 1);
        assert!(!KINDS.contains(&WORD_LIMIT_KIND));
        let (_, problems) = shared::parse_allowlist("docs/design/a.md word-limit 1\n", &KINDS);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn a_page_in_no_markdown_attr_is_detected() {
        let build = "t(\n    markdown_siblings = [\"//:README.md\", \":b.md\"],\n    markdown_srcs = [\"a.md\"] + glob([\"guide/*.md\"]),\n    other = [\"c.md\"],\n)\n";
        let found = hits(&[
            ("README.md", "x\n"),
            ("docs/BUILD.bazel", build),
            ("docs/a.md", "x\n"),
            ("docs/b.md", "x\n"),
            ("docs/c.md", "x\n"),
            ("docs/guide/d.md", "x\n"),
            ("docs/guide/deep/e.md", "x\n"),
            ("docs/testdata/f.md", "x\n"),
        ]);
        assert_eq!(
            found,
            [hit("docs/c.md", "not-linted", 1), hit("docs/guide/deep/e.md", "not-linted", 1)]
        );
    }

    #[test]
    fn a_link_to_an_untracked_file_is_detected() {
        let page = "[a](b.md) [b](../README.md#top) [c](gone.md) [d](https://x.test/y.md) [e](#here)\n[f](sub/) [g](../../out.md) [h](/docs/b.md) [i](<gone two.md>)\n```\n[j](fenced.md)\n```\n";
        let found = hits(&[
            LINTED,
            ("README.md", "x\n"),
            ("docs/a.md", page),
            ("docs/b.md", "x\n"),
            ("docs/sub/c.md", "x\n"),
        ]);
        assert_eq!(found, [hit("docs/a.md", "dead-link", 3)]);
    }
}
