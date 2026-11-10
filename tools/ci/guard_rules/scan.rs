//! Counted source-scan hits, the shrink-only allowlist that bounds them, and the Starlark text helpers the scans share.

use std::collections::BTreeMap;
use std::path::Path;

/// Hits keyed by file path and kind.
pub type Counts = BTreeMap<(String, String), usize>;

/// Path components that mark a file as test-only or vendored.
const SKIPPED_DIRS: [&str; 5] = ["fixtures", "patches", "testdata", "tests", "third_party"];

/// File name endings that mark a Starlark file as test-only.
const TEST_SUFFIXES: [&str; 2] = ["_test.bzl", "_tests.bzl"];

/// Adds `n` hits of `kind` in `path`.
pub fn add(counts: &mut Counts, path: &str, kind: &str, n: usize) {
    if n > 0 {
        *counts
            .entry((path.to_owned(), kind.to_owned()))
            .or_default() += n;
    }
}

/// True when `path` is a test, a fixture, a patch or vendored.
pub fn is_skipped(path: &str) -> bool {
    path.split('/').any(|part| SKIPPED_DIRS.contains(&part))
        || TEST_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
}

/// True when `path` names a BUILD, MODULE.bazel or .bzl file.
pub fn is_starlark(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "BUILD" || name == "BUILD.bazel" || name == "MODULE.bazel" || name.ends_with(".bzl")
}

/// Every file under `root` as a sorted `(relative path, text)` list.
pub fn tree(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|error| panic!("read_dir {dir:?}: {error}"));
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("path under the snapshot root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {path:?}: {error}"));
                out.push((rel, text));
            }
        }
    }
    out.sort();
    out
}

/// The allowlist lines for `counts`.
pub fn render(counts: &Counts) -> String {
    counts
        .iter()
        .map(|((path, kind), count)| format!("{path} {kind} {count}\n"))
        .collect()
}

/// Parses `<path> <kind> <count>` lines, or names every malformed, unsorted or repeated line.
pub fn parse_allowlist(text: &str, kinds: &[&str]) -> Result<Counts, Vec<String>> {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous: Option<(String, String)> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let fields: Vec<&str> = line.split(' ').collect();
        let count = match fields.as_slice() {
            [path, kind, count] if !path.is_empty() && kinds.contains(kind) => count
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0 && n.to_string() == *count),
            _ => None,
        };
        let Some(count) = count else {
            problems.push(format!(
                "line {number} is not `<path> <kind> <count>`: {line:?}"
            ));
            continue;
        };
        let key = (fields[0].to_owned(), fields[1].to_owned());
        if previous.as_ref().is_some_and(|before| *before >= key) {
            problems.push(format!(
                "line {number} is not sorted after the line above it: {line:?}"
            ));
        }
        previous = Some(key.clone());
        counts.insert(key, count);
    }
    if problems.is_empty() {
        Ok(counts)
    } else {
        Err(problems)
    }
}

/// Every way `found` and the allowlist text disagree; empty when they are equal.
pub fn problems(
    found: &Counts,
    allowlist: &str,
    kinds: &[&str],
    allowlist_path: &str,
) -> Vec<String> {
    let allowed = match parse_allowlist(allowlist, kinds) {
        Ok(allowed) => allowed,
        Err(problems) => {
            return problems
                .into_iter()
                .map(|problem| format!("{allowlist_path}: {problem}"))
                .collect();
        }
    };
    let mut out = Vec::new();
    for ((path, kind), count) in found {
        match allowed.get(&(path.clone(), kind.clone())) {
            Some(limit) if limit == count => {}
            Some(limit) if limit > count => out.push(format!(
                "{path} has {count} {kind}, {allowlist_path} still says {limit}: lower the line"
            )),
            Some(limit) => out.push(format!(
                "{path} has {count} {kind}, {allowlist_path} allows {limit}"
            )),
            None => out.push(format!(
                "{path} has {count} {kind}, {allowlist_path} allows none"
            )),
        }
    }
    for (path, kind) in allowed.keys() {
        if !found.contains_key(&(path.clone(), kind.clone())) {
            out.push(format!(
                "{allowlist_path}: spare line `{path} {kind}` has no hit: delete it"
            ));
        }
    }
    out
}

/// Starlark text with comments blanked, and the same text with string contents blanked too.
pub struct Masked {
    pub code: String,
    pub mask: String,
}

/// Blanks comments in `code` and comments plus string contents in `mask`, keeping every byte offset.
pub fn masked(text: &str) -> Masked {
    let bytes = text.as_bytes();
    let mut code = bytes.to_vec();
    let mut mask = bytes.to_vec();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'#' => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    code[at] = b' ';
                    mask[at] = b' ';
                    at += 1;
                }
            }
            quote @ (b'"' | b'\'') => {
                let triple = bytes[at..].starts_with(&[quote, quote, quote]);
                let width = if triple { 3 } else { 1 };
                at += width;
                while at < bytes.len() {
                    if bytes[at] == b'\\' && at + 1 < bytes.len() {
                        mask[at] = b'_';
                        mask[at + 1] = b'_';
                        at += 2;
                    } else if (triple && bytes[at..].starts_with(&[quote, quote, quote]))
                        || (!triple && (bytes[at] == quote || bytes[at] == b'\n'))
                    {
                        at += width;
                        break;
                    } else {
                        if bytes[at] != b'\n' {
                            mask[at] = b'_';
                        }
                        at += 1;
                    }
                }
            }
            _ => at += 1,
        }
    }
    Masked {
        code: String::from_utf8_lossy(&code).into_owned(),
        mask: String::from_utf8(mask).expect("masked text is UTF-8"),
    }
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// One call: its arguments with strings intact and with strings blanked.
pub struct CallArgs<'a> {
    pub code: &'a str,
    pub mask: &'a str,
}

/// The arguments of every call of `name` in `source`. A `name` that starts with a dot matches any receiver; any other must stand alone or follow an underscore prefix, and never follows `def`.
pub fn calls<'a>(source: &'a Masked, name: &str) -> Vec<CallArgs<'a>> {
    let mask = source.mask.as_bytes();
    let needle = format!("{name}(");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = source.mask[from..].find(&needle) {
        let start = from + found;
        let open = start + needle.len();
        from = open;
        if !name.starts_with('.') {
            let before = &source.mask[..start];
            let prefix = before.trim_end_matches('_');
            if prefix
                .bytes()
                .last()
                .is_some_and(|byte| is_ident(byte) || byte == b'.')
                || prefix.trim_end_matches(' ').ends_with("def")
            {
                continue;
            }
        }
        let mut depth = 1usize;
        let mut end = open;
        while end < mask.len() && depth > 0 {
            match mask[end] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let close = if depth == 0 { end - 1 } else { end };
        out.push(CallArgs {
            code: &source.code[open..close],
            mask: &source.mask[open..close],
        });
    }
    out
}

impl CallArgs<'_> {
    /// The text after `key =` up to the next top-level comma, trimmed, or None when the call has no such keyword argument.
    pub fn keyword(&self, key: &str) -> Option<&str> {
        let mask = self.mask.as_bytes();
        let mut depth = 0usize;
        let mut starts = vec![0usize];
        for (at, byte) in mask.iter().enumerate() {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => starts.push(at + 1),
                _ => {}
            }
        }
        starts.push(mask.len() + 1);
        starts.windows(2).find_map(|pair| {
            let (start, end) = (pair[0], (pair[1] - 1).min(mask.len()));
            let (name, _) = self.mask[start..end].split_once('=')?;
            let value_at = start + name.len() + 1;
            (name.trim() == key && !self.mask[value_at..end].starts_with('='))
                .then(|| self.code[value_at..end].trim())
        })
    }
}
