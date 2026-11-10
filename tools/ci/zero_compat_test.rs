//! BUILD, .bzl and Rust files outside third_party and patches carry no deprecated attr, in-module alias, re-export file, compat word or version conditional, outside tools/ci/zero_compat_allowlist.txt.

use std::collections::BTreeMap;

use serde_json as _;
use shared::{Counts, Piece, Segment, add, is_starlark, under};
use starlark_syntax as _;
use yaml_serde as _;

#[path = "guards/shared.rs"]
mod shared;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/zero_compat_allowlist.txt";

/// This file, which spells every word it bans.
const SELF: &str = "tools/ci/zero_compat_test.rs";

/// The name of this guard, which is no compat word.
const OWN_NAME: &str = "zero_compat";

/// The kinds an allowlist line may name.
const KINDS: [&str; 5] = [
    "alias",
    "compat-word",
    "deprecated-attr",
    "reexport",
    "version-conditional",
];

/// Words that name compatibility code.
const COMPAT_WORDS: [&str; 5] = ["legacy", "deprecated", "shim", "backward", "compat"];

/// Directory names whose files are vendored.
const SKIPPED_DIRS: [&str; 2] = ["third_party", "patches"];

/// Starlark spellings that branch on the Bazel version or its features.
const VERSION_PROBES: [&str; 3] = ["native.bazel_version", "bazel_features.", "hasattr(native,"];

/// Rust spellings that compare two values.
const COMPARISONS: [&str; 9] = [" < ", " > ", " <= ", " >= ", ".cmp(", ".lt(", ".gt(", ".le(", ".ge("];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 3000;

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// How often a compat word stands as one word of an identifier, string or comment in `text`.
fn compat_words(text: &str) -> usize {
    let mut count = 0;
    let mut word = String::new();
    let mut previous_lower = false;
    for letter in text.replace(OWN_NAME, "").chars().chain(std::iter::once(' ')) {
        let breaks = !letter.is_ascii_alphabetic() || (letter.is_ascii_uppercase() && previous_lower);
        if breaks {
            count += usize::from(COMPAT_WORDS.contains(&word.as_str()));
            word.clear();
        }
        if letter.is_ascii_alphabetic() {
            word.push(letter.to_ascii_lowercase());
        }
        previous_lower = letter.is_ascii_lowercase();
    }
    count
}

/// How often `needle` stands in `text` with no identifier byte before it.
fn count_from_boundary(text: &str, needle: &str) -> usize {
    let bytes = text.as_bytes();
    text.match_indices(needle)
        .filter(|(start, _)| *start == 0 || !ident_byte(bytes[start - 1]))
        .count()
}

/// The code of `segments` with each string as `""` and no comment.
fn skeleton(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| match segment.piece {
            Piece::Code => segment.text.as_str(),
            Piece::Comment => "",
            Piece::Text | Piece::Doc => "\"\"",
        })
        .collect()
}

/// How often Starlark sets `deprecation` or says deprecated in a docstring or `doc` value.
fn starlark_deprecations(segments: &[Segment]) -> usize {
    let code = skeleton(segments);
    let bytes = code.as_bytes();
    let set = code
        .match_indices("deprecation")
        .filter(|(start, word)| {
            let rest = code[start + word.len()..].trim_start();
            (*start == 0 || !ident_byte(bytes[start - 1])) && rest.starts_with('=') && !rest.starts_with("==")
        })
        .count();
    let said = segments
        .iter()
        .filter(|segment| segment.piece == Piece::Doc && segment.text.to_lowercase().contains("deprecated"))
        .count();
    set + said
}

/// Whether the label `text` names a target of this module.
fn in_module(text: &str) -> bool {
    ["//", ":", "@rules_dx//", "@//", "@@//"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
        || !text.starts_with('@')
}

/// How many `alias(` calls set `actual` to a label of this module.
fn in_module_aliases(segments: &[Segment]) -> usize {
    let mut count = 0;
    let mut depth = 0_i32;
    let mut alias_depth = None;
    let mut actual = false;
    for segment in segments {
        match segment.piece {
            Piece::Comment => {}
            Piece::Text | Piece::Doc => {
                count += usize::from(actual && in_module(&segment.text));
                actual = false;
            }
            Piece::Code => {
                let text = segment.text.as_str();
                for (index, byte) in text.bytes().enumerate() {
                    if byte == b'(' {
                        if alias_depth.is_none() && text[..index].ends_with("alias") {
                            let start = index - "alias".len();
                            let dotted = start > 0 && text.as_bytes()[start - 1] == b'.';
                            if dotted || start == 0 || !ident_byte(text.as_bytes()[start - 1]) {
                                alias_depth = Some(depth);
                            }
                        }
                        depth += 1;
                    } else if byte == b')' {
                        depth -= 1;
                        if alias_depth == Some(depth) {
                            alias_depth = None;
                        }
                    }
                }
                let tail = text.trim_end();
                actual = alias_depth.is_some_and(|open| open + 1 == depth)
                    && (tail.ends_with("actual =") || tail.ends_with("actual="));
            }
        }
    }
    count
}

fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|part| {
            !part.is_empty() && part.bytes().all(ident_byte) && !part.as_bytes()[0].is_ascii_digit()
        })
}

/// Whether a .bzl file holds only `load` statements and name-to-name assignments, at least one of each.
fn reexports(segments: &[Segment]) -> bool {
    let code = skeleton(segments);
    let (mut loads, mut assigns, mut depth) = (0, 0, 0_i32);
    let mut statement = String::new();
    for letter in code.chars().chain(std::iter::once('\n')) {
        match letter {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if letter != '\n' || depth > 0 {
            statement.push(letter);
            continue;
        }
        let text = statement.trim();
        if text.starts_with("load(") {
            loads += 1;
        } else if let Some((left, right)) = text.split_once('=') {
            if !is_name(left.trim()) || !is_name(right.trim()) || matches!(right.trim(), "True" | "False" | "None") {
                return false;
            }
            assigns += 1;
        } else if !text.is_empty() && text != "\"\"" {
            return false;
        }
        statement.clear();
    }
    loads > 0 && assigns > 0
}

/// Whether `path` is `<lang>/defs.bzl` or `<lang>/extensions.bzl`.
fn public_entry(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), [_, "defs.bzl" | "extensions.bzl"])
}

/// How many Rust lines compare against a rules_dx version.
fn rust_version_comparisons(segments: &[Segment]) -> usize {
    skeleton(segments)
        .lines()
        .filter(|line| {
            line.to_lowercase().contains("rules_dx_version") && COMPARISONS.iter().any(|sign| line.contains(sign))
        })
        .count()
}

/// Counts every hit of every kind in `files`.
fn scan(files: &BTreeMap<String, String>) -> Counts {
    let mut counts = Counts::new();
    for (path, text) in files {
        if under(path, &SKIPPED_DIRS) || path == SELF {
            continue;
        }
        if is_starlark(path) {
            let segments = shared::starlark_segments(text);
            let code = skeleton(&segments);
            add(&mut counts, path, "compat-word", compat_words(text));
            add(&mut counts, path, "deprecated-attr", starlark_deprecations(&segments));
            add(&mut counts, path, "alias", in_module_aliases(&segments));
            let probes = VERSION_PROBES.iter().map(|probe| count_from_boundary(&code, probe)).sum();
            add(&mut counts, path, "version-conditional", probes);
            let reexport = path.ends_with(".bzl") && !public_entry(path) && reexports(&segments);
            add(&mut counts, path, "reexport", usize::from(reexport));
        } else if path.ends_with(".rs") {
            let segments = shared::rust_segments(text);
            add(&mut counts, path, "compat-word", compat_words(text));
            add(&mut counts, path, "deprecated-attr", skeleton(&segments).matches("#[deprecated").count());
            add(&mut counts, path, "version-conditional", rust_version_comparisons(&segments));
        }
    }
    counts
}

#[test]
fn no_compat_code_outside_the_allowlist() {
    let files = shared::tree(&dx_testing::source_snapshot());
    assert!(files.len() >= MIN_FILES, "only {} files in the snapshot", files.len());
    shared::settle(&scan(&files), "DX_ALLOWLIST", ALLOWLIST, &KINDS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(entries: &[(&str, &str)]) -> Vec<(String, String, usize)> {
        let files: BTreeMap<String, String> = entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        scan(&files)
            .into_iter()
            .map(|((path, kind), count)| (path, kind, count))
            .collect()
    }

    fn hit(path: &str, kind: &str, count: usize) -> (String, String, usize) {
        (path.to_owned(), kind.to_owned(), count)
    }

    #[test]
    fn deprecated_attrs_are_detected() {
        let found = hits(&[
            ("a/BUILD.bazel", "t(name = \"a\", deprecation = \"gone\")\nu(x = undeprecation == 1)\n"),
            ("a/r.bzl", "r = rule(attrs = {\"x\": attr.string(doc = \"Deprecated: use y.\")})\n"),
            ("a/x.rs", "#[deprecated(note = \"x\")]\nfn f() {}\nconst T: &str = \"#[deprecated]\";\n"),
        ]);
        assert_eq!(
            found,
            [
                hit("a/BUILD.bazel", "deprecated-attr", 1),
                hit("a/r.bzl", "compat-word", 1),
                hit("a/r.bzl", "deprecated-attr", 1),
                hit("a/x.rs", "compat-word", 2),
                hit("a/x.rs", "deprecated-attr", 1),
            ]
        );
    }

    #[test]
    fn in_module_aliases_are_detected_and_select_and_external_ones_are_not() {
        let build = "alias(name = \"a\", actual = \"//x:y\")\nalias(\n    name = \"b\",\n    actual = \":z\",\n)\nnative.alias(name = \"c\", actual = \"@rules_dx//x\")\nalias(name = \"d\", actual = select({\"//c:x\": \"//x:y\"}))\nalias(name = \"e\", actual = \"@other//x\")\nmy_alias(name = \"f\", actual = \"//x:y\")\nt(name = \"g\", actual = \"//x:y\")\n";
        let found = hits(&[("a/BUILD.bazel", build), ("third_party/BUILD.bazel", build)]);
        assert_eq!(found, [hit("a/BUILD.bazel", "alias", 3)]);
    }

    #[test]
    fn reexport_files_are_detected_outside_the_public_entry_points() {
        let reexport = "\"\"\"Doc.\"\"\"\n\nload(\n    \"//a:b.bzl\",\n    _x = \"x\",\n)\n\n# note\nx = _x\ny = _x.y\n";
        let found = hits(&[
            ("a/private/r.bzl", reexport),
            ("a/defs.bzl", reexport),
            ("a/extensions.bzl", reexport),
            ("a/private/call.bzl", "load(\"//a:b.bzl\", \"x\")\ny = x()\n"),
            ("a/private/flag.bzl", "load(\"//a:b.bzl\", \"x\")\ny = True\n"),
            ("a/private/loads.bzl", "load(\"//a:b.bzl\", \"x\")\n"),
            ("a/private/consts.bzl", "x = y\n"),
        ]);
        assert_eq!(found, [hit("a/private/r.bzl", "reexport", 1)]);
    }

    #[test]
    fn compat_words_are_detected_as_whole_words_of_identifiers_strings_and_comments() {
        assert_eq!(compat_words("legacy_mode LegacyMode isLegacy \"a shim\" // backward compat"), 6);
        assert_eq!(compat_words("target_compatible_with compatibility_level bazel_compatibility"), 0);
        assert_eq!(compat_words("shims backwards legacyx compatible incompat"), 0);
        assert_eq!(compat_words("zero_compat_test zero_compat_allowlist.txt"), 0);
        let found = hits(&[
            ("a/x.bzl", "X = [\"legacy\"]  # deprecated\n"),
            ("a/x.rs", "fn shim_path() {}\n"),
            ("patches/x.bzl", "LEGACY = 1\n"),
            (SELF, "const X: &str = \"legacy\";\n"),
        ]);
        assert_eq!(found, [hit("a/x.bzl", "compat-word", 2), hit("a/x.rs", "compat-word", 1)]);
    }

    #[test]
    fn version_conditionals_are_detected() {
        let bzl = "load(\"@bazel_features//:features.bzl\", \"bazel_features\")\nif bazel_features.cc.x:\n    pass\nif native.bazel_version > \"8\":\n    pass\nif hasattr(native, \"x\"):  # hasattr(native, y)\n    pass\nmy_bazel_features.x\n";
        let rust = "fn f(rules_dx_version: u32) -> bool { rules_dx_version >= 3 }\nconst RULES_DX_VERSION: &str = \"1 < 2\";\n// RULES_DX_VERSION < 2\n";
        let found = hits(&[("a/x.bzl", bzl), ("a/x.rs", rust)]);
        assert_eq!(
            found,
            [hit("a/x.bzl", "version-conditional", 3), hit("a/x.rs", "version-conditional", 1)]
        );
    }
}
