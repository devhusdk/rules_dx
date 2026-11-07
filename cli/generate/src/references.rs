//! Files a root config names, such as vale `StylesPath` or ruff `extend`, as labels `//.dx` can bind.

use std::collections::BTreeSet;

use crate::GenerateError;
use crate::tools_gen::ToolConfig;

/// A referenced file: the label to bind, and the root file to export when it sits in the root package.
pub struct Reference {
    pub label: String,
    pub export: Option<String>,
}

/// The workspace paths `text` names that `resolve` would look up.
pub fn targets(config: &ToolConfig, text: &str) -> Vec<String> {
    config
        .references
        .iter()
        .filter_map(|spec| spec.split_once(':'))
        .flat_map(|(syntax, key)| values(syntax, key, text))
        .filter_map(|value| normalize(&value))
        .filter(|path| !path.starts_with(".dx/"))
        .collect()
}

/// Every `.dx/` path `text` names, without a leading `./`.
pub fn dx_paths(text: &str) -> Vec<String> {
    let is_path = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-');
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(".dx/") {
        let start = rest[..at]
            .char_indices()
            .rfind(|(_, c)| !is_path(*c))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let end = rest[at..]
            .find(|c: char| !is_path(c))
            .map_or(rest.len(), |i| at + i);
        let token = &rest[start..end];
        let token = token.strip_prefix("./").unwrap_or(token);
        let token = token.trim_end_matches(['/', '.']);
        if token.starts_with(".dx/") && !out.iter().any(|path| path == token) {
            out.push(token.to_owned());
        }
        rest = &rest[end..];
    }
    out
}

pub fn resolve(
    config: &ToolConfig,
    file: &str,
    text: &str,
    files: &BTreeSet<String>,
) -> Result<Vec<Reference>, GenerateError> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for spec in config.references {
        let Some((syntax, key)) = spec.split_once(':') else {
            continue;
        };
        for value in values(syntax, key, text) {
            let problem = |problem: &str| GenerateError::Reference {
                tool: config.id.to_owned(),
                file: file.to_owned(),
                path: value.clone(),
                problem: problem.to_owned(),
            };
            let path = normalize(&value).ok_or_else(|| problem("it points outside the repo"))?;
            if path.starts_with(".dx/") {
                continue;
            }
            let prefix = format!("{path}/");
            let matched: Vec<&String> = files
                .iter()
                .filter(|candidate| **candidate == path || candidate.starts_with(&prefix))
                .filter(|candidate| !is_build(candidate))
                .collect();
            if matched.is_empty() {
                return Err(problem("no such file or directory in the workspace"));
            }
            found.extend(matched.into_iter().cloned());
        }
    }
    Ok(found
        .into_iter()
        .map(|path| match package(&path, files) {
            Some(package) => Reference {
                label: format!("//{package}:{}", &path[package.len() + 1..]),
                export: None,
            },
            None => Reference {
                label: format!("//:{path}"),
                export: Some(path),
            },
        })
        .collect())
}

fn is_build(path: &str) -> bool {
    matches!(path.rsplit('/').next(), Some("BUILD" | "BUILD.bazel"))
}

/// The nearest enclosing package of `path`, or `None` for the root package.
fn package<'a>(path: &'a str, files: &BTreeSet<String>) -> Option<&'a str> {
    let mut dir = path;
    while let Some((parent, _)) = dir.rsplit_once('/') {
        if files.contains(&format!("{parent}/BUILD.bazel"))
            || files.contains(&format!("{parent}/BUILD"))
        {
            return Some(parent);
        }
        dir = parent;
    }
    None
}

/// A workspace-relative path without `.` and `..` parts, or `None` when it leaves the workspace.
fn normalize(value: &str) -> Option<String> {
    let value = value.replace('\\', "/");
    if value.starts_with('/') || value.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn values(syntax: &str, key: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with(['#', ';']) {
            continue;
        }
        if syntax == "toml" && line.starts_with('[') {
            break;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        let value = value.trim();
        match syntax {
            "ini" => out.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_owned),
            ),
            "toml" => {
                let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''));
                if let Some((inner, _)) = quote.and_then(|quote| value[1..].split_once(quote)) {
                    out.push(inner.to_owned());
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn dx_paths_finds_each_dx_token_once() {
        let text = "export { default } from \"./.dx/config/eslint/extra.mjs\";\n\
                    extend = '.dx/config/ruff/base.toml' # .dx/config/ruff/base.toml\n\
                    other.dx/x ../.dx/up é.dx/pins";
        assert_eq!(
            super::dx_paths(text),
            [
                ".dx/config/eslint/extra.mjs",
                ".dx/config/ruff/base.toml",
                ".dx/pins"
            ]
        );
    }
}
