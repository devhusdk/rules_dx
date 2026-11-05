//! Places rustc's `unused_crate_dependencies` warnings on the BUILD file entry that causes them.
//!
//! The Rust aspect writes one `dx_crate_deps` line per crate next to the compiler diagnostics. It
//! maps each dependency's crate name to its target name, which is all the BUILD file needs.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, Suggestion, TextPosition, ToolSeverity};

pub const RULE_ID: &str = "unused_crate_dependencies";

const DEPS_MESSAGE_TYPE: &str = "dx_crate_deps";

#[derive(Debug, Deserialize)]
struct CrateDeps {
    package: String,
    target: String,
    #[serde(rename = "crate")]
    crate_name: String,
    deps: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    message: String,
    code: Option<Code>,
}

#[derive(Debug, Deserialize)]
struct Code {
    code: String,
}

/// A dependency of one rule that its sources never use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnusedDependency {
    pub package: String,
    pub target: String,
    pub dependency: String,
    pub crate_name: String,
}

/// Reads the diagnostics and `dx_crate_deps` lines and names every unused dependency once.
pub fn parse_unused_dependencies(stream: &[u8]) -> Result<Vec<UnusedDependency>, ParseError> {
    let tool = "rustc";
    let text = std::str::from_utf8(stream).map_err(|err| ParseError::Shape {
        tool,
        detail: err.to_string(),
    })?;
    let mut crates = Vec::new();
    let mut unused: Vec<(String, String)> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|err| ParseError::Shape {
                tool,
                detail: format!("unparsable line: {err}"),
            })?;
        match value
            .get("$message_type")
            .and_then(serde_json::Value::as_str)
        {
            Some(DEPS_MESSAGE_TYPE) => {
                crates.push(serde_json::from_value::<CrateDeps>(value).map_err(|err| {
                    ParseError::Shape {
                        tool,
                        detail: format!("malformed {DEPS_MESSAGE_TYPE}: {err}"),
                    }
                })?);
            }
            Some("diagnostic") => {
                let diagnostic: Diagnostic =
                    serde_json::from_value(value).map_err(|err| ParseError::Shape {
                        tool,
                        detail: format!("malformed diagnostic: {err}"),
                    })?;
                if diagnostic.code.is_some_and(|code| code.code == RULE_ID) {
                    if let Some(names) = unused_names(&diagnostic.message) {
                        unused.push(names);
                    }
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for (extern_crate, in_crate) in unused {
        for entry in crates.iter().filter(|entry| entry.crate_name == in_crate) {
            if let Some(dependency) = entry.deps.get(&extern_crate) {
                let item = UnusedDependency {
                    package: entry.package.clone(),
                    target: entry.target.clone(),
                    dependency: dependency.clone(),
                    crate_name: extern_crate.clone(),
                };
                if !found.contains(&item) {
                    found.push(item);
                }
            }
        }
    }
    Ok(found)
}

/// Splits "external crate `a` unused in `b`: ..." into the dependency and the crate using it.
fn unused_names(message: &str) -> Option<(String, String)> {
    let rest = message.strip_prefix("external crate `")?;
    let (extern_crate, rest) = rest.split_once('`')?;
    let rest = rest.strip_prefix(" unused in `")?;
    let (in_crate, _) = rest.split_once('`')?;
    Some((extern_crate.to_owned(), in_crate.to_owned()))
}

/// Finds the `deps` entry of the unused dependency in the BUILD file text and offers its removal.
pub fn place_unused_dependency(
    unused: &UnusedDependency,
    build_file: &str,
    text: &str,
) -> Option<FileFinding> {
    place_dependency_finding(unused, build_file, text, "rustc", RULE_ID)
}

/// Like `place_unused_dependency`, reported by `tool_id` under `rule_id`.
pub fn place_dependency_finding(
    unused: &UnusedDependency,
    build_file: &str,
    text: &str,
    tool_id: &str,
    rule_id: &str,
) -> Option<FileFinding> {
    let (start, end) = dep_literal(text, &unused.target, &unused.dependency)?;
    let (cut_start, cut_end) = removal(text, start, end);
    let message = format!(
        "`{}` is in deps of `{}` but no source uses it",
        unused.crate_name, unused.target
    );
    Some(FileFinding {
        file: build_file.to_owned(),
        finding: Finding {
            tool_id: tool_id.to_owned(),
            rule_id: rule_id.to_owned(),
            message,
            severity: ToolSeverity::Warning,
            start: position(text, start),
            end: Some(position(text, end)),
            suggestions: vec![Suggestion {
                start: cut_start as u64,
                end: cut_end as u64,
                replacement: Vec::new(),
            }],
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    })
}

/// Byte range of the quoted entry of `deps` in the rule named `target` that names `dependency`.
fn dep_literal(text: &str, target: &str, dependency: &str) -> Option<(usize, usize)> {
    let rule = text.find(&format!("name = \"{target}\""))?;
    let rule_end = text[rule..].find("\n)").map_or(text.len(), |at| rule + at);
    let mut from = rule;
    while let Some(at) = text[from..rule_end].find("deps = [") {
        let key = from + at;
        from = key + "deps = [".len();
        let before = text[..key].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let list_end = text[from..rule_end].find(']').map_or(rule_end, |at| from + at);
        return literals(&text[from..list_end], from)
            .into_iter()
            .find(|(start, end)| target_name(&text[start + 1..end - 1]) == dependency);
    }
    None
}

/// The quoted strings of a list body, as byte ranges including the quotes.
fn literals(body: &str, offset: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    let mut comment = false;
    for (at, c) in body.char_indices() {
        match (open, c) {
            (Some(start), '"') => {
                out.push((offset + start, offset + at + 1));
                open = None;
            }
            (Some(_), _) => {}
            (None, '\n') => comment = false,
            (None, '#') => comment = true,
            (None, '"') if !comment => open = Some(at),
            _ => {}
        }
    }
    out
}

fn target_name(label: &str) -> &str {
    match label.rsplit_once(':') {
        Some((_, name)) => name,
        None => label.rsplit('/').next().unwrap_or(label).trim_start_matches('@'),
    }
}

/// The bytes to delete: the whole line when the entry is alone on it, else the entry and its comma.
fn removal(text: &str, start: usize, end: usize) -> (usize, usize) {
    let mut cut_end = end;
    if text[cut_end..].starts_with(',') {
        cut_end += 1;
    }
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[cut_end..]
        .find('\n')
        .map_or(text.len(), |at| cut_end + at + 1);
    let alone = text[line_start..start].trim().is_empty()
        && text[cut_end..line_end].trim().is_empty();
    if alone {
        return (line_start, line_end);
    }
    let spaces = text[cut_end..]
        .chars()
        .take_while(|c| *c == ' ')
        .count();
    (start, cut_end + spaces)
}

fn position(text: &str, at: usize) -> TextPosition {
    let before = &text[..at];
    let line = before.matches('\n').count() as u64 + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |tail| tail.chars().count()) as u64
        + 1;
    TextPosition { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD: &str = concat!(
        "rust_library(\n",
        "    name = \"lib\",\n",
        "    srcs = [\"lib.rs\"],\n",
        "    deps = [\n",
        "        \"//util:used\",\n",
        "        \"@crates//:serde\",\n",
        "    ],\n",
        ")\n",
    );

    fn stream(extern_crate: &str) -> String {
        format!(
            concat!(
                r#"{{"$message_type":"dx_crate_deps","package":"p","target":"lib","crate":"lib","deps":{{"used":"used","serde":"serde"}}}}"#,
                "\n",
                r#"{{"$message_type":"diagnostic","message":"external crate `{}` unused in `lib`: remove the dependency or add `use {} as _;`","code":{{"code":"unused_crate_dependencies"}},"level":"warning","spans":[],"children":[]}}"#,
            ),
            extern_crate, extern_crate
        )
    }

    #[test]
    fn names_the_unused_dependency() {
        let found = parse_unused_dependencies(stream("serde").as_bytes()).expect("parsed");
        assert_eq!(
            found,
            vec![UnusedDependency {
                package: "p".into(),
                target: "lib".into(),
                dependency: "serde".into(),
                crate_name: "serde".into(),
            }]
        );
    }

    #[test]
    fn ignores_crates_without_a_dep_entry() {
        let found = parse_unused_dependencies(stream("other").as_bytes()).expect("parsed");
        assert!(found.is_empty());
    }

    #[test]
    fn places_the_finding_on_the_deps_line_and_removes_the_line() {
        let unused = parse_unused_dependencies(stream("serde").as_bytes()).expect("parsed")[0].clone();
        let placed = place_unused_dependency(&unused, "p/BUILD.bazel", BUILD).expect("placed");
        assert_eq!(placed.file, "p/BUILD.bazel");
        assert_eq!(placed.finding.start, TextPosition { line: 6, column: 9 });
        let cut = &placed.finding.suggestions[0];
        let mut fixed = BUILD.to_owned();
        fixed.replace_range(cut.start as usize..cut.end as usize, "");
        assert!(!fixed.contains("serde"));
        assert!(fixed.contains("\"//util:used\",\n    ],"));
    }

    #[test]
    fn removes_an_inline_entry_with_its_comma() {
        let build = "rust_library(name = \"lib\", deps = [\"//a:a\", \"//b:b\"])\n";
        let unused = UnusedDependency {
            package: "p".into(),
            target: "lib".into(),
            dependency: "a".into(),
            crate_name: "a".into(),
        };
        let placed = place_unused_dependency(&unused, "BUILD", build).expect("placed");
        let cut = &placed.finding.suggestions[0];
        let mut fixed = build.to_owned();
        fixed.replace_range(cut.start as usize..cut.end as usize, "");
        assert_eq!(fixed, "rust_library(name = \"lib\", deps = [\"//b:b\"])\n");
    }

    #[test]
    fn leaves_proc_macro_deps_alone() {
        let build = "rust_library(name = \"lib\", proc_macro_deps = [\"//a:a\"])\n";
        let unused = UnusedDependency {
            package: "p".into(),
            target: "lib".into(),
            dependency: "a".into(),
            crate_name: "a".into(),
        };
        assert!(place_unused_dependency(&unused, "BUILD", build).is_none());
    }
}
