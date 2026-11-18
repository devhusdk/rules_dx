//! BUILD, .bzl and Rust files outside `third_party` and patches carry no deprecated attr, in-module alias, re-export file, compat word or version conditional, outside `tools/ci/zero_compat_allowlist.txt`.
//!
//! Two forms of deprecation are allowed, and both must be recorded in `api/deprecations.txt`: a call of
//! `deprecated(what, remove_in)` from `core/private/deprecate.bzl`, and native `deprecation =` on an `alias`
//! whose message says `removed in rules_dx X.Y.Z`. Once the module version reaches `remove_in`, the test fails
//! until the deprecated name is deleted.

use std::collections::BTreeMap;
use std::path::Path;

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

/// The one file that may define the deprecation helper.
const DEPRECATE: &str = "core/private/deprecate.bzl";

/// How a file loads the helper; it says the helper's name once, which is no compat word.
const DEPRECATE_LOAD: &str = "load(\"//core/private:deprecate.bzl\", \"deprecated\")";

/// What an alias deprecation message says before its removal version.
const ALIAS_REMOVAL: &str = "removed in rules_dx ";

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
const COMPARISONS: [&str; 9] = [
    " < ", " > ", " <= ", " >= ", ".cmp(", ".lt(", ".gt(", ".le(", ".ge(",
];

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
    for letter in text
        .replace(OWN_NAME, "")
        .chars()
        .chain(std::iter::once(' '))
    {
        let breaks =
            !letter.is_ascii_alphabetic() || (letter.is_ascii_uppercase() && previous_lower);
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

/// How often Starlark sets `deprecation` outside an `alias` call or says deprecated in a docstring or `doc` value.
fn starlark_deprecations(segments: &[Segment]) -> usize {
    let code = skeleton(segments);
    let bytes = code.as_bytes();
    let set = code
        .match_indices("deprecation")
        .filter(|(start, word)| {
            let rest = code
                .get(start + word.len()..)
                .expect("string slice within bounds")
                .trim_start();
            (*start == 0 || !ident_byte(bytes[start - 1]))
                && rest.starts_with('=')
                && !rest.starts_with("==")
        })
        .count();
    let on_aliases = calls(segments, "alias")
        .iter()
        .filter(|call| call.keywords.contains_key("deprecation"))
        .count();
    let said = segments
        .iter()
        .filter(|segment| {
            segment.piece == Piece::Doc && segment.text.to_lowercase().contains("deprecated")
        })
        .count();
    set.saturating_sub(on_aliases) + said
}

/// How often Starlark says a compat word, not counting loads and calls of the deprecation helper.
fn starlark_compat_words(text: &str, segments: &[Segment]) -> usize {
    let code = skeleton(segments);
    let helper_calls =
        count_from_boundary(&code, "deprecated(") - count_from_boundary(&code, "def deprecated(");
    compat_words(text)
        .saturating_sub(text.matches(DEPRECATE_LOAD).count())
        .saturating_sub(helper_calls)
}

/// One `name(...)` call: its string-literal arguments, positional and by keyword.
#[derive(Debug, Default, PartialEq, Eq)]
struct Call {
    positional: Vec<String>,
    keywords: BTreeMap<String, String>,
}

/// Every call of `name` (also as `native.name`) with the arguments that are a single string literal.
fn calls(segments: &[Segment], name: &str) -> Vec<Call> {
    let mut found: Vec<Call> = Vec::new();
    let mut depth = 0_i32;
    let mut open: Option<i32> = None;
    // The code of the current argument so far, before any string in it.
    let mut pending = String::new();
    for segment in segments {
        let direct = open.is_some_and(|level| level + 1 == depth);
        match segment.piece {
            Piece::Comment => {}
            Piece::Text | Piece::Doc => {
                if let (true, Some(call)) = (direct, found.last_mut()) {
                    let before = pending.trim();
                    if before.is_empty() {
                        call.positional.push(segment.text.clone());
                    } else if let Some(keyword) = before.strip_suffix('=') {
                        let keyword = keyword.trim();
                        if is_name(keyword) && !keyword.contains('.') {
                            call.keywords
                                .insert(keyword.to_owned(), segment.text.clone());
                        }
                    }
                    // Anything after the string makes the argument more than one literal.
                    pending.push('?');
                }
            }
            Piece::Code => {
                let text = segment.text.as_str();
                let bytes = text.as_bytes();
                for (index, byte) in text.bytes().enumerate() {
                    let direct = open.is_some_and(|level| level + 1 == depth);
                    match byte {
                        b'(' | b'[' | b'{' => {
                            let head = text.get(..index).unwrap_or_default();
                            let start = index.saturating_sub(name.len());
                            if byte == b'('
                                && open.is_none()
                                && head.ends_with(name)
                                && (start == 0
                                    || bytes[start - 1] == b'.'
                                    || !ident_byte(bytes[start - 1]))
                            {
                                open = Some(depth);
                                found.push(Call::default());
                                pending.clear();
                            } else if direct {
                                pending.push(char::from(byte));
                            }
                            depth += 1;
                        }
                        b')' | b']' | b'}' => {
                            depth -= 1;
                            if open == Some(depth) {
                                open = None;
                            } else if open.is_some_and(|level| level + 1 == depth) {
                                pending.push(char::from(byte));
                            }
                        }
                        b',' if direct => pending.clear(),
                        _ if direct => pending.push(char::from(byte)),
                        _ => {}
                    }
                }
            }
        }
    }
    found
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
                        if alias_depth.is_none()
                            && text
                                .get(..index)
                                .expect("string slice within bounds")
                                .ends_with("alias")
                        {
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
            if !is_name(left.trim())
                || !is_name(right.trim())
                || matches!(right.trim(), "True" | "False" | "None")
            {
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

/// How many Rust lines compare against a `rules_dx` version.
fn rust_version_comparisons(segments: &[Segment]) -> usize {
    skeleton(segments)
        .lines()
        .filter(|line| {
            line.to_lowercase().contains("rules_dx_version")
                && COMPARISONS.iter().any(|sign| line.contains(sign))
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
            if path != DEPRECATE {
                add(
                    &mut counts,
                    path,
                    "compat-word",
                    starlark_compat_words(text, &segments),
                );
                add(
                    &mut counts,
                    path,
                    "deprecated-attr",
                    starlark_deprecations(&segments),
                );
            }
            // A deprecated alias is how a renamed target keeps its old label for one minor release.
            let deprecated_aliases = calls(&segments, "alias")
                .iter()
                .filter(|call| {
                    call.keywords.contains_key("deprecation")
                        && call
                            .keywords
                            .get("actual")
                            .is_some_and(|actual| in_module(actual))
                })
                .count();
            add(
                &mut counts,
                path,
                "alias",
                in_module_aliases(&segments).saturating_sub(deprecated_aliases),
            );
            let probes = VERSION_PROBES
                .iter()
                .map(|probe| count_from_boundary(&code, probe))
                .sum();
            add(&mut counts, path, "version-conditional", probes);
            let reexport = Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("bzl"))
                && !public_entry(path)
                && reexports(&segments);
            add(&mut counts, path, "reexport", usize::from(reexport));
        } else if Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
        {
            let segments = shared::rust_segments(text);
            add(&mut counts, path, "compat-word", compat_words(text));
            add(
                &mut counts,
                path,
                "deprecated-attr",
                skeleton(&segments).matches("#[deprecated").count(),
            );
            add(
                &mut counts,
                path,
                "version-conditional",
                rust_version_comparisons(&segments),
            );
        }
    }
    counts
}

/// A `MAJOR.MINOR.PATCH` version as numbers, so `0.10.0` sorts after `0.9.0`.
fn semver(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Every deprecation in Starlark outside vendored code, as `(what, remove_in)`, and the problems that keep one from being read.
fn deprecations(files: &BTreeMap<String, String>) -> (Vec<(String, String)>, Vec<String>) {
    let (mut found, mut problems) = (Vec::new(), Vec::new());
    for (path, text) in files {
        if under(path, &SKIPPED_DIRS) || path == DEPRECATE || !is_starlark(path) {
            continue;
        }
        let segments = shared::starlark_segments(text);
        for call in calls(&segments, "deprecated") {
            let what = call.keywords.get("what").or(call.positional.first());
            let remove_in = call.keywords.get("remove_in").or(call.positional.get(1));
            match (what, remove_in) {
                (Some(what), Some(remove_in)) => found.push((what.clone(), remove_in.clone())),
                (None, _) | (_, None) => problems.push(format!(
                    "{path}: pass deprecated() its what and remove_in as string literals"
                )),
            }
        }
        let package = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        for call in calls(&segments, "alias") {
            let Some(message) = call.keywords.get("deprecation") else {
                continue;
            };
            let name = call.keywords.get("name").map_or("?", String::as_str);
            let remove_in = message
                .split_once(ALIAS_REMOVAL)
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .map(|word| word.trim_end_matches(['.', ',', ';', ')']));
            match remove_in {
                Some(version) => found.push((format!("//{package}:{name}"), version.to_owned())),
                None => problems.push(format!(
                    "{path}: alias {name} must say '{ALIAS_REMOVAL}X.Y.Z' in its deprecation message"
                )),
            }
        }
    }
    found.sort();
    (found, problems)
}

/// Compares the deprecations in code with the `remove_in=X deprecated_since=Y what` lines of the snapshot and the module version.
fn deprecation_problems(found: &[(String, String)], snapshot: &str, module: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(now) = semver(module) else {
        return vec![format!(
            "MODULE.bazel version {module:?} is not MAJOR.MINOR.PATCH"
        )];
    };
    let mut recorded = Vec::new();
    for line in snapshot.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.splitn(3, ' ');
        let entry = (
            words
                .next()
                .and_then(|word| word.strip_prefix("remove_in=")),
            words
                .next()
                .and_then(|word| word.strip_prefix("deprecated_since=")),
            words.next(),
        );
        let (Some(remove_in), Some(since), Some(what)) = entry else {
            problems.push(format!(
                "api/deprecations.txt: {line:?} is not 'remove_in=X.Y.Z deprecated_since=X.Y.Z <what>'"
            ));
            continue;
        };
        match (semver(since), semver(remove_in)) {
            (Some(since_version), Some(remove_version)) => {
                if since_version >= remove_version {
                    problems.push(format!(
                        "{what}: deprecated_since {since} is not before remove_in {remove_in}"
                    ));
                }
                if since_version > now {
                    problems.push(format!(
                        "{what}: deprecated_since {since} is after the module version {module}"
                    ));
                }
                if now >= remove_version {
                    problems.push(format!(
                        "{what}: the module is at {module}, which reaches remove_in {remove_in}; delete it and its api/deprecations.txt line"
                    ));
                }
            }
            // A line with a bad version is reported once; it is not also
            // matched against code, so it cannot cascade into a second error.
            (None, _) | (_, None) => {
                problems.push(format!(
                    "api/deprecations.txt: {line:?} has a version that is not MAJOR.MINOR.PATCH"
                ));
                continue;
            }
        }
        recorded.push((what.to_owned(), remove_in.to_owned()));
    }
    recorded.sort();
    for (what, remove_in) in found {
        if !recorded.contains(&(what.clone(), remove_in.clone())) {
            problems.push(format!(
                "{what} is deprecated in code but not recorded; add 'remove_in={remove_in} deprecated_since={module} {what}' to api/deprecations.txt"
            ));
        }
    }
    for (what, remove_in) in &recorded {
        if !found.contains(&(what.clone(), remove_in.clone())) {
            problems.push(format!(
                "api/deprecations.txt records {what} (remove_in={remove_in}), which code no longer deprecates; delete the line"
            ));
        }
    }
    problems
}

#[test]
fn deprecations_are_recorded_and_gone_by_their_removal_version() {
    let files = shared::tree(&dx_testing::source_snapshot());
    let module_file = files
        .get("MODULE.bazel")
        .expect("MODULE.bazel is in the source snapshot");
    let module_segments = shared::starlark_segments(module_file);
    let module = calls(&module_segments, "module")
        .into_iter()
        .find_map(|call| call.keywords.get("version").cloned())
        .expect("MODULE.bazel sets module(version = ...)");
    let path = dx_testing::rlocation(dx_testing::env_text("DX_DEPRECATIONS").trim())
        .unwrap_or_else(|error| panic!("DX_DEPRECATIONS: {error}"));
    let (found, mut problems) = deprecations(&files);
    problems.extend(deprecation_problems(
        &found,
        &dx_testing::read_text(&path),
        &module,
    ));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_compat_code_outside_the_allowlist() {
    let files = shared::tree(&dx_testing::source_snapshot());
    assert!(
        files.len() >= MIN_FILES,
        "only {} files in the snapshot",
        files.len()
    );
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
            (
                "a/BUILD.bazel",
                "t(name = \"a\", deprecation = \"gone\")\nu(x = undeprecation == 1)\n",
            ),
            (
                "a/r.bzl",
                "r = rule(attrs = {\"x\": attr.string(doc = \"Deprecated: use y.\")})\n",
            ),
            (
                "a/x.rs",
                "#[deprecated(note = \"x\")]\nfn f() {}\nconst T: &str = \"#[deprecated]\";\n",
            ),
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
            (
                "a/private/call.bzl",
                "load(\"//a:b.bzl\", \"x\")\ny = x()\n",
            ),
            (
                "a/private/flag.bzl",
                "load(\"//a:b.bzl\", \"x\")\ny = True\n",
            ),
            ("a/private/loads.bzl", "load(\"//a:b.bzl\", \"x\")\n"),
            ("a/private/consts.bzl", "x = y\n"),
        ]);
        assert_eq!(found, [hit("a/private/r.bzl", "reexport", 1)]);
    }

    #[test]
    fn compat_words_are_detected_as_whole_words_of_identifiers_strings_and_comments() {
        assert_eq!(
            compat_words("legacy_mode LegacyMode isLegacy \"a shim\" // backward compat"),
            6
        );
        assert_eq!(
            compat_words("target_compatible_with compatibility_level bazel_compatibility"),
            0
        );
        assert_eq!(
            compat_words("shims backwards legacyx compatible incompat"),
            0
        );
        assert_eq!(
            compat_words("zero_compat_test zero_compat_allowlist.txt"),
            0
        );
        let found = hits(&[
            ("a/x.bzl", "X = [\"legacy\"]  # deprecated\n"),
            ("a/x.rs", "fn shim_path() {}\n"),
            ("patches/x.bzl", "LEGACY = 1\n"),
            (SELF, "const X: &str = \"legacy\";\n"),
        ]);
        assert_eq!(
            found,
            [
                hit("a/x.bzl", "compat-word", 2),
                hit("a/x.rs", "compat-word", 1)
            ]
        );
    }

    #[test]
    fn the_deprecation_helper_and_deprecated_aliases_are_allowed() {
        let bzl = "load(\"//core/private:deprecate.bzl\", \"deprecated\")\n\ndef old(**kwargs):\n    deprecated(\"old (use new)\", \"0.2.0\")\n    new(**kwargs)\n";
        let build = "alias(\n    name = \"old\",\n    actual = \":new\",\n    deprecation = \"use :new; removed in rules_dx 0.2.0\",\n)\nt(name = \"x\", deprecation = \"gone\")\n";
        let found = hits(&[
            ("a/defs.bzl", bzl),
            ("a/BUILD.bazel", build),
            (
                DEPRECATE,
                "def deprecated(what, remove_in):\n    \"\"\"Deprecated.\"\"\"\n",
            ),
            ("b/x.bzl", "def deprecated(x):\n    pass\n"),
        ]);
        assert_eq!(
            found,
            [
                hit("a/BUILD.bazel", "deprecated-attr", 1),
                hit("b/x.bzl", "compat-word", 1),
            ]
        );
        let files: BTreeMap<String, String> = [
            ("a/defs.bzl", bzl),
            ("a/BUILD.bazel", build),
            (
                "b/BUILD.bazel",
                "alias(name = \"c\", actual = \":d\", deprecation = \"gone\")\n",
            ),
            (
                "b/x.bzl",
                "deprecated(what = \"w\", remove_in = \"1.0.0\")\ndeprecated(NAME, \"1.0.0\")\n",
            ),
        ]
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
        let (found, problems) = deprecations(&files);
        assert_eq!(
            found,
            [
                ("//a:old".to_owned(), "0.2.0".to_owned()),
                ("old (use new)".to_owned(), "0.2.0".to_owned()),
                ("w".to_owned(), "1.0.0".to_owned()),
            ]
        );
        assert_eq!(
            problems,
            [
                "b/BUILD.bazel: alias c must say 'removed in rules_dx X.Y.Z' in its deprecation message",
                "b/x.bzl: pass deprecated() its what and remove_in as string literals",
            ]
        );
    }

    #[test]
    fn deprecations_fail_once_the_module_reaches_remove_in() {
        let found = [("old (use new)".to_owned(), "0.10.0".to_owned())];
        let snapshot = "# comment\nremove_in=0.10.0 deprecated_since=0.9.0 old (use new)\n";
        assert!(deprecation_problems(&found, snapshot, "0.9.3").is_empty());
        assert_eq!(
            deprecation_problems(&found, snapshot, "0.10.0"),
            [
                "old (use new): the module is at 0.10.0, which reaches remove_in 0.10.0; delete it and its api/deprecations.txt line"
            ]
        );
        assert_eq!(
            deprecation_problems(&found, "", "0.9.0"),
            [
                "old (use new) is deprecated in code but not recorded; add 'remove_in=0.10.0 deprecated_since=0.9.0 old (use new)' to api/deprecations.txt"
            ]
        );
        assert_eq!(
            deprecation_problems(&[], snapshot, "0.9.0"),
            [
                "api/deprecations.txt records old (use new) (remove_in=0.10.0), which code no longer deprecates; delete the line"
            ]
        );
        assert_eq!(
            deprecation_problems(&[], "remove_in=1 deprecated_since=0.1.0 x\n", "0.1.0"),
            [
                "api/deprecations.txt: \"remove_in=1 deprecated_since=0.1.0 x\" has a version that is not MAJOR.MINOR.PATCH"
            ]
        );
    }

    #[test]
    fn version_conditionals_are_detected() {
        let bzl = "load(\"@bazel_features//:features.bzl\", \"bazel_features\")\nif bazel_features.cc.x:\n    pass\nif native.bazel_version > \"8\":\n    pass\nif hasattr(native, \"x\"):  # hasattr(native, y)\n    pass\nmy_bazel_features.x\n";
        let rust = "fn f(rules_dx_version: u32) -> bool { rules_dx_version >= 3 }\nconst RULES_DX_VERSION: &str = \"1 < 2\";\n// RULES_DX_VERSION < 2\n";
        let found = hits(&[("a/x.bzl", bzl), ("a/x.rs", rust)]);
        assert_eq!(
            found,
            [
                hit("a/x.bzl", "version-conditional", 3),
                hit("a/x.rs", "version-conditional", 1)
            ]
        );
    }
}
