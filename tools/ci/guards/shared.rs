//! Allowlist counting, snapshot reading and source lexing shared by the source-scan guards.

use std::collections::BTreeMap;
use std::path::Path;

/// Occurrences keyed by path and kind.
pub type Counts = BTreeMap<(String, String), usize>;

/// What one stretch of a source file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    Code,
    Comment,
    Text,
    Doc,
}

/// One stretch of a source file: code, a comment without its marker, or a string without its quotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub piece: Piece,
    pub text: String,
}

/// Adds `count` hits of `kind` in `path`, ignoring zero.
pub fn add(counts: &mut Counts, path: &str, kind: &str, count: usize) {
    if count > 0 {
        *counts.entry((path.to_owned(), kind.to_owned())).or_default() += count;
    }
}

/// Reads `<path> <kind> <count>` lines; `kinds` are the kinds a line may name.
pub fn parse_allowlist(text: &str, kinds: &[&str]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, kind, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count.parse::<usize>().unwrap_or(0);
        if count == 0 || path.is_empty() || !kinds.contains(kind) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        counts.insert(((*path).to_owned(), (*kind).to_owned()), count);
    }
    (counts, problems)
}

/// Every way the tree and the allowlist disagree.
pub fn differences(found: &Counts, allowed: &Counts) -> Vec<String> {
    let mut problems = Vec::new();
    for ((path, kind), count) in found {
        match allowed.get(&(path.clone(), kind.clone())) {
            None => problems.push(format!("{path}: {count} new {kind}; remove it")),
            Some(limit) if count > limit => problems.push(format!(
                "{path}: {count} {kind}, allowlist holds {limit}; remove the new one"
            )),
            Some(limit) if count < limit => problems.push(format!(
                "{path}: {count} {kind}, allowlist holds {limit}; lower the line to {count}"
            )),
            Some(_) => {}
        }
    }
    for (path, kind) in allowed.keys() {
        if !found.contains_key(&(path.clone(), kind.clone())) {
            problems.push(format!("{path}: no {kind} left; delete the line"));
        }
    }
    problems
}

/// The allowlist text that matches `found`, leaving out the kinds no line may name.
pub fn render(found: &Counts, kinds: &[&str]) -> String {
    let mut lines: Vec<String> = found
        .iter()
        .filter(|((_, kind), _)| kinds.contains(&kind.as_str()))
        .map(|((path, kind), count)| format!("{path} {kind} {count}\n"))
        .collect();
    lines.sort();
    lines.concat()
}

/// Fails when `found` and the allowlist named by `variable` disagree.
pub fn settle(found: &Counts, variable: &str, allowlist: &str, kinds: &[&str]) {
    let key = dx_testing::env_text(variable);
    let path = dx_testing::rlocation(key.trim()).unwrap_or_else(|error| panic!("{variable}: {error}"));
    let (allowed, mut problems) = parse_allowlist(&dx_testing::read_text(&path), kinds);
    problems.extend(differences(found, &allowed));
    if problems.is_empty() {
        return;
    }
    if let Some(dir) = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR") {
        let name = allowlist.rsplit('/').next().unwrap_or(allowlist);
        dx_testing::write_text(&Path::new(&dir).join(name), &render(found, kinds));
    }
    panic!(
        "{allowlist} and the tree disagree ({} problems):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read_dir {dir:?}: {error}"));
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).expect("path under the snapshot root");
            let rel: Vec<String> = rel
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.insert(rel.join("/"), dx_testing::read_text(&path));
        }
    }
}

/// Every file under `root` by its `/`-separated relative path.
pub fn tree(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Whether one directory name of `path` is in `names`.
pub fn under(path: &str, names: &[&str]) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    parts.iter().any(|part| names.contains(part))
}

/// Whether `path` is a BUILD, MODULE.bazel or .bzl file.
pub fn is_starlark(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(name, "BUILD" | "BUILD.bazel" | "MODULE.bazel")
        || name.ends_with(".bzl")
        || name.ends_with(".BUILD")
        || name.ends_with(".BUILD.bazel")
}

fn push(out: &mut Vec<Segment>, piece: Piece, text: &str) {
    if !text.is_empty() || piece != Piece::Code {
        out.push(Segment {
            piece,
            text: text.to_owned(),
        });
    }
}

/// The end of the line that holds byte `from`.
fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| from + offset)
}

/// Splits Starlark into code, `#` comments and strings; a string is `Doc` when it is a docstring or a `doc =` value.
pub fn starlark_segments(source: &str) -> Vec<Segment> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let (mut index, mut code_start, mut line_start) = (0, 0, 0);
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                index += 1;
                line_start = index;
            }
            b'#' => {
                push(&mut out, Piece::Code, &source[code_start..index]);
                let end = line_end(bytes, index);
                push(&mut out, Piece::Comment, &source[index + 1..end]);
                index = end;
                code_start = end;
            }
            quote @ (b'"' | b'\'') => {
                push(&mut out, Piece::Code, &source[code_start..index]);
                let triple = bytes[index..].starts_with(&[quote, quote, quote]);
                let width = if triple { 3 } else { 1 };
                let before = source[line_start..index].trim();
                let doc = (triple && before.is_empty())
                    || before.ends_with("doc =")
                    || before.ends_with("doc=");
                let start = index + width;
                let mut end = start;
                while end < bytes.len() {
                    if bytes[end] == b'\\' {
                        end += 2;
                    } else if bytes[end..].starts_with(&[quote, quote, quote][..width]) {
                        break;
                    } else {
                        end += 1;
                    }
                }
                let end = end.min(bytes.len());
                let piece = if doc { Piece::Doc } else { Piece::Text };
                push(&mut out, piece, &source[start..end]);
                index = (end + width).min(bytes.len());
                code_start = index;
                if let Some(newline) = source[start.min(index)..index].rfind('\n') {
                    line_start = start + newline + 1;
                }
            }
            _ => index += 1,
        }
    }
    push(&mut out, Piece::Code, &source[code_start..]);
    out
}

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The byte after the raw string that opens at `index`, or `None` when no raw string opens there.
fn raw_string(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    if bytes[index] != b'r' || (index > 0 && ident_byte(bytes[index - 1])) {
        return None;
    }
    let hashes = bytes[index + 1..].iter().take_while(|byte| **byte == b'#').count();
    let open = index + 1 + hashes;
    if bytes.get(open) != Some(&b'"') {
        return None;
    }
    let mut end = open + 1;
    while end < bytes.len() {
        if bytes[end] == b'"' && bytes[end + 1..].iter().take(hashes).filter(|byte| **byte == b'#').count() == hashes {
            return Some((open + 1, end));
        }
        end += 1;
    }
    Some((open + 1, bytes.len()))
}

/// The length of the char literal that opens at `index`, or `None` for a lifetime.
fn char_literal(source: &str, index: usize) -> Option<usize> {
    let rest = &source[index + 1..];
    if let Some(escaped) = rest.strip_prefix('\\') {
        return escaped.get(1..)?.find('\'').map(|offset| offset + 4);
    }
    let first = rest.chars().next()?;
    rest[first.len_utf8()..]
        .starts_with('\'')
        .then_some(first.len_utf8() + 2)
}

/// Splits Rust into code, comments and string literals.
pub fn rust_segments(source: &str) -> Vec<Segment> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let (mut index, mut code_start) = (0, 0);
    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            push(&mut out, Piece::Code, &source[code_start..index]);
            let end = line_end(bytes, index);
            push(&mut out, Piece::Comment, &source[index + 2..end]);
            index = end;
            code_start = end;
        } else if bytes[index..].starts_with(b"/*") {
            push(&mut out, Piece::Code, &source[code_start..index]);
            let (mut end, mut depth) = (index + 2, 1);
            while end < bytes.len() && depth > 0 {
                if bytes[end..].starts_with(b"/*") {
                    depth += 1;
                    end += 2;
                } else if bytes[end..].starts_with(b"*/") {
                    depth -= 1;
                    end += 2;
                } else {
                    end += 1;
                }
            }
            let end = end.min(bytes.len());
            push(&mut out, Piece::Comment, source[index + 2..end].trim_end_matches("*/"));
            index = end;
            code_start = end;
        } else if let Some((start, end)) = raw_string(bytes, index) {
            push(&mut out, Piece::Code, &source[code_start..index]);
            push(&mut out, Piece::Text, &source[start..end]);
            let hashes = start - index - 2;
            index = (end + 1 + hashes).min(bytes.len());
            code_start = index;
        } else if bytes[index] == b'"' {
            push(&mut out, Piece::Code, &source[code_start..index]);
            let mut end = index + 1;
            while end < bytes.len() && bytes[end] != b'"' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            let end = end.min(bytes.len());
            push(&mut out, Piece::Text, &source[index + 1..end]);
            index = (end + 1).min(bytes.len());
            code_start = index;
        } else if bytes[index] == b'\'' {
            index += char_literal(source, index).unwrap_or(1);
        } else {
            index += 1;
        }
    }
    push(&mut out, Piece::Code, &source[code_start..]);
    out
}

fn name_match(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|skip| name_match(rest, &name[skip..])),
        Some((byte, rest)) => name.first() == Some(byte) && name_match(rest, &name[1..]),
    }
}

fn parts_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| parts_match(rest, &path[skip..])),
        Some((first, rest)) => path.split_first().is_some_and(|(name, tail)| {
            name_match(first.as_bytes(), name.as_bytes()) && parts_match(rest, tail)
        }),
    }
}

/// Whether Bazel glob `pattern` matches `path`.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    parts_match(&pattern, &path)
}

/// How often `word` stands in `text` with no letter, digit or `_` on either side.
pub fn count_word(text: &str, word: &str) -> usize {
    let bytes = text.as_bytes();
    text.match_indices(word)
        .filter(|(start, _)| {
            let end = start + word.len();
            (*start == 0 || !ident_byte(bytes[start - 1])) && (end == bytes.len() || !ident_byte(bytes[end]))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(segments: &[Segment]) -> Vec<(Piece, &str)> {
        segments
            .iter()
            .map(|segment| (segment.piece, segment.text.as_str()))
            .collect()
    }

    fn counts(entries: &[(&str, &str, usize)]) -> Counts {
        let mut out = Counts::new();
        for (path, kind, count) in entries {
            add(&mut out, path, kind, *count);
        }
        out
    }

    #[test]
    fn a_spare_allowlist_line_fails() {
        let (allowed, problems) = parse_allowlist("a.bzl alias 1\nb.bzl alias 2\n", &["alias"]);
        assert!(problems.is_empty(), "{problems:?}");
        let found = counts(&[("b.bzl", "alias", 2), ("c.bzl", "alias", 0)]);
        assert_eq!(
            differences(&found, &allowed),
            ["a.bzl: no alias left; delete the line"]
        );
    }

    #[test]
    fn a_count_above_or_below_the_line_fails() {
        let (allowed, _) = parse_allowlist("a.bzl alias 2\nb.bzl alias 2\n", &["alias"]);
        let found = counts(&[("a.bzl", "alias", 3), ("b.bzl", "alias", 1), ("c.bzl", "alias", 1)]);
        let problems = differences(&found, &allowed);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("allowlist holds 2; remove the new one"));
        assert!(problems[1].contains("lower the line to 1"));
        assert!(problems[2].contains("1 new alias"));
    }

    #[test]
    fn malformed_unsorted_and_unknown_kind_lines_fail() {
        let (_, problems) = parse_allowlist("b alias 1\na alias 1\na other 1\na alias 0\na alias\n", &["alias"]);
        assert_eq!(problems.len(), 6, "{problems:?}");
    }

    #[test]
    fn render_round_trips_and_leaves_out_unlisted_kinds() {
        let found = counts(&[("a-b/x", "alias", 1), ("a/x", "alias", 2), ("a/x", "word-limit", 1)]);
        let text = render(&found, &["alias"]);
        assert_eq!(text, "a-b/x alias 1\na/x alias 2\n");
        let (allowed, problems) = parse_allowlist(&text, &["alias"]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(allowed.len(), 2);
    }

    #[test]
    fn the_tree_reads_back_what_render_wrote() {
        let scratch = dx_testing::mkscratch("settle").expect("scratch dir");
        let found = counts(&[("a", "alias", 1)]);
        let file = scratch.join("list.txt");
        dx_testing::write_text(&file, &render(&found, &["alias"]));
        assert_eq!(tree(&scratch).get("list.txt").map(String::as_str), Some("a alias 1\n"));
        let (allowed, _) = parse_allowlist(&dx_testing::read_text(&file), &["alias"]);
        assert!(differences(&found, &allowed).is_empty());
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
    fn rust_splits_comments_strings_chars_and_lifetimes() {
        let source = "// top\nfn f<'a>(x: &'a str) -> char { let _ = \"a // b\\\" c\"; let _ = r#\"raw \" // x\"#; '\"' } /* block */ let q = '\\''; // end\n";
        let segments = rust_segments(source);
        let found = pieces(&segments);
        assert!(found.contains(&(Piece::Comment, " top")), "{found:?}");
        assert!(found.contains(&(Piece::Text, "a // b\\\" c")), "{found:?}");
        assert!(found.contains(&(Piece::Text, "raw \" // x")), "{found:?}");
        assert!(found.contains(&(Piece::Comment, " block ")), "{found:?}");
        assert!(found.contains(&(Piece::Comment, " end")), "{found:?}");
        assert_eq!(found.iter().filter(|(piece, _)| *piece == Piece::Comment).count(), 3);
        assert_eq!(found.iter().filter(|(piece, _)| *piece == Piece::Text).count(), 2);
    }

    #[test]
    fn globs_words_and_paths_match() {
        assert!(glob_match("fixtures/**", "fixtures/a/b.txt"));
        assert!(glob_match("**/*.md", "a/b/c.md"));
        assert!(glob_match("*.md", "c.md"));
        assert!(!glob_match("*.md", "a/c.md"));
        assert!(!glob_match("fixtures/*.txt", "fixtures/a/b.txt"));
        assert_eq!(count_word("legacy legacy_x xlegacy (legacy)", "legacy"), 2);
        assert!(under("a/fixtures/b.md", &["fixtures"]));
        assert!(!under("a/fixtures", &["fixtures"]));
        assert!(is_starlark("a/BUILD.bazel") && is_starlark("x.bzl") && is_starlark("MODULE.bazel"));
        assert!(!is_starlark("a/b.rs"));
    }
}
