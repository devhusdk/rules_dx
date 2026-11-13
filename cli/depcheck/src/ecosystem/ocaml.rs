use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Packages that build, document or format the project and never appear in a `libraries` field.
const TOOLS: [&str; 8] = [
    "ocaml",
    "dune",
    "odoc",
    "ocamlformat",
    "ocamlfind",
    "ocaml-lsp-server",
    "merlin",
    "utop",
];

/// Reads the `depends` and `depopts` lists of an opam file. A `with-test` filter marks a
/// dependency dev, `build` marks it build, and `depopts` entries are optional.
pub fn parse_ocaml_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let mut deps = BTreeMap::new();
    for (key, optional) in [("depends", false), ("depopts", true)] {
        for (name, filter) in list_entries(&text, key) {
            if TOOLS.contains(&name.as_str()) {
                continue;
            }
            let category = if has_flag(&filter, &["with-test", "with-dev-setup", "with-doc"]) {
                "dev"
            } else if has_flag(&filter, &["build"]) {
                "build"
            } else {
                "prod"
            };
            put(
                &mut deps,
                name.clone(),
                DepInfo {
                    spec: opam_spec(&filter),
                    category: category.to_owned(),
                    optional,
                    platform: false,
                    raw: name,
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
    }
    Ok(deps)
}

/// Maps each package of an `opam.locked` file to its pinned version.
pub fn parse_ocaml_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let version_re = regex::Regex::new(r#"=\s*"([^"]+)""#).map_err(DepcheckError::LockRegex)?;
    Ok(list_entries(&text, "depends")
        .into_iter()
        .map(|(name, filter)| {
            let version = version_re
                .captures(&filter)
                .map_or(String::new(), |caps| caps[1].to_owned());
            (name, version)
        })
        .collect())
}

/// The `"name" {filter}` entries of the `key: [ ... ]` list, with the filter text between braces.
fn list_entries(text: &str, key: &str) -> Vec<(String, String)> {
    let text = strip_comments(text);
    let Some(list) = find_list(&text, key).and_then(|start| text.get(start..)) else {
        return Vec::new();
    };
    let chars: Vec<char> = list.chars().collect();
    let mut out = Vec::new();
    let mut rest = chars.as_slice();
    while let Some((&c, tail)) = rest.split_first() {
        if c == ']' {
            break;
        }
        if c != '"' {
            rest = tail;
            continue;
        }
        let name_len = tail.iter().position(|&ch| ch == '"').unwrap_or(tail.len());
        let (name, after) = tail.split_at_checked(name_len).unwrap_or((tail, &[]));
        let name: String = name.iter().collect();
        rest = after.get(1..).unwrap_or_default();
        let look = rest
            .iter()
            .position(|ch| !ch.is_whitespace())
            .and_then(|ws| rest.get(ws..))
            .unwrap_or_default();
        let mut filter = String::new();
        if look.first() == Some(&'{') {
            let mut depth = 0;
            let close = look
                .iter()
                .position(|&ch| {
                    match ch {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    ch == '}' && depth == 0
                })
                .unwrap_or(look.len());
            filter = look.get(1..close).unwrap_or_default().iter().collect();
            rest = look.get(close + 1..).unwrap_or_default();
        }
        out.push((name, filter));
    }
    out
}

/// Byte offset just past the `[` of `key: [`.
fn find_list(text: &str, key: &str) -> Option<usize> {
    let re = regex::Regex::new(&format!(r"(?m)^\s*{}\s*:\s*\[", regex::escape(key))).ok()?;
    re.find(text).map(|found| found.end())
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| line.split_once('#').map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

fn has_flag(filter: &str, flags: &[&str]) -> bool {
    filter
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .any(|word| flags.contains(&word))
}

/// Turns `>= "1.0" & < "2.0"` into `>= 1.0, < 2.0`; no version constraint means any.
fn opam_spec(filter: &str) -> String {
    let re = regex::Regex::new(r#"(>=|<=|!=|<|>|=)\s*"([^"]+)""#);
    let Ok(re) = re else {
        return "*".to_owned();
    };
    let parts: Vec<String> = re
        .captures_iter(filter)
        .map(|caps| match &caps[1] {
            "=" => format!("== {}", &caps[2]),
            op => format!("{op} {}", &caps[2]),
        })
        .collect();
    if parts.is_empty() {
        "*".to_owned()
    } else {
        parts.join(", ")
    }
}
