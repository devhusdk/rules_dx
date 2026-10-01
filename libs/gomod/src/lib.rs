//! One reader for `go.mod` and `go.sum`, shared by every Go dependency tool.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;

use gomod_parser::{GoMod, ModuleReplacement, Replacement};

const DIRECTIVES: [&str; 10] = [
    "exclude",
    "godebug",
    "go",
    "ignore",
    "module",
    "replace",
    "require",
    "retract",
    "tool",
    "toolchain",
];

/// One `require` entry with the trailing comment its line carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoRequirement {
    pub module: String,
    pub version: String,
    pub indirect: bool,
    pub marker: Option<String>,
}

/// What a `replace` directive redirects a module to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoReplacementTarget {
    FilePath(String),
    Module { module: String, version: String },
}

/// One `replace` directive, keyed by the module path it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoReplacement {
    pub module: String,
    pub version: Option<String>,
    pub target: GoReplacementTarget,
}

/// The `go.mod` directives a dependency reader needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoModFile {
    pub module: String,
    pub go: Option<String>,
    pub require: Vec<GoRequirement>,
    pub replace: Vec<GoReplacement>,
}

/// Reads a `go.mod`, failing on syntax the Go toolchain rejects.
pub fn parse_mod(text: &str) -> Result<GoModFile, String> {
    let parsed = GoMod::from_str(text).map_err(|detail| format!("invalid go.mod: {detail}"))?;
    let mut markers = requirement_markers(text);
    let require = parsed
        .require
        .into_iter()
        .map(|entry| {
            let module = entry.module.module_path;
            let marker = markers.get_mut(&module).and_then(VecDeque::pop_front);
            GoRequirement {
                marker,
                module,
                version: entry.module.version,
                indirect: entry.indirect,
            }
        })
        .collect();
    let replace = parsed.replace.into_iter().map(replacement).collect();
    Ok(GoModFile {
        module: parsed.module,
        go: parsed.go,
        require,
        replace,
    })
}

/// Reads a `go.sum` into module paths and versions, keeping the first of each pair.
pub fn parse_sum(text: &str) -> BTreeMap<String, String> {
    let mut packages = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let (Some(module), Some(version)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        let version = version.strip_suffix("/go.mod").unwrap_or(version);
        packages
            .entry(module.to_owned())
            .or_insert_with(|| version.to_owned());
    }
    packages
}

fn replacement(entry: ModuleReplacement) -> GoReplacement {
    let target = match entry.replacement {
        Replacement::FilePath(path) => GoReplacementTarget::FilePath(path),
        Replacement::Module(module) => GoReplacementTarget::Module {
            module: module.module_path,
            version: module.version,
        },
    };
    GoReplacement {
        module: entry.module_path,
        version: entry.version,
        target,
    }
}

/// Pairs every `require` line with the comment that line carries.
fn requirement_markers(text: &str) -> BTreeMap<String, VecDeque<String>> {
    let mut markers: BTreeMap<String, VecDeque<String>> = BTreeMap::new();
    let mut block: Option<&str> = None;
    for line in text.lines() {
        let (code, comment) = line.split_once("//").unwrap_or((line, ""));
        let comment = comment.trim();
        let tokens = code.split_whitespace().collect::<Vec<_>>();
        let Some(keyword) = tokens.first().copied() else {
            continue;
        };
        if keyword == ")" {
            block = None;
            continue;
        }
        if DIRECTIVES.contains(&keyword) {
            block = if tokens.get(1) == Some(&"(") {
                Some(keyword)
            } else {
                None
            };
            if keyword == "require" && tokens.len() > 2 {
                record_marker(&mut markers, tokens[1], comment);
            }
            continue;
        }
        if block == Some("require") && tokens.len() == 2 {
            record_marker(&mut markers, tokens[0], comment);
        }
    }
    markers
}

fn record_marker(markers: &mut BTreeMap<String, VecDeque<String>>, module: &str, comment: &str) {
    if comment.is_empty() {
        return;
    }
    markers
        .entry(module.to_owned())
        .or_default()
        .push_back(comment.to_owned());
}

#[cfg(test)]
mod tests;
