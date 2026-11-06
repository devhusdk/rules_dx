use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::normalize_lower;

fn dep_info(raw: &str, spec: &str, category: &str, optional: bool) -> DepInfo {
    DepInfo {
        spec: if spec.trim().is_empty() || spec.contains(" or ") {
            "*".to_owned()
        } else {
            spec.trim().to_owned()
        },
        category: category.to_owned(),
        optional,
        platform: false,
        raw: raw.to_owned(),
        peer: false,
        sha256: String::new(),
        package: String::new(),
    }
}

/// The body of `def deps do ... end` in a `mix.exs`.
fn mix_deps_block(text: &str) -> Option<String> {
    let head = regex::Regex::new(r"(?m)^\s*defp?\s+deps(?:\(\s*\))?\s+do\b").ok()?;
    let start = head.find(text)?.end();
    let mut block = String::new();
    for line in text[start..].lines() {
        if line.trim() == "end" {
            break;
        }
        block.push_str(line);
        block.push('\n');
    }
    Some(block)
}

pub fn parse_elixir_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let entry_re =
        regex::Regex::new(r#"\{\s*:([A-Za-z_][A-Za-z0-9_]*)\s*(?:,\s*"([^"]*)")?([^{}]*)\}"#)
            .map_err(DepcheckError::ManifestRegex)?;
    let optional_re =
        regex::Regex::new(r"\boptional:\s*true\b").map_err(DepcheckError::ManifestRegex)?;
    let only_re =
        regex::Regex::new(r"\bonly:\s*(?::dev|:test|\[)").map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    let block = mix_deps_block(&text).unwrap_or_default();
    let without_comments: String = block
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    for caps in entry_re.captures_iter(&without_comments) {
        let rest = caps.get(3).map_or("", |m| m.as_str());
        let category = if only_re.is_match(rest) {
            "dev"
        } else {
            "prod"
        };
        deps.insert(
            normalize_lower(&caps[1]),
            dep_info(
                &caps[1],
                caps.get(2).map_or("", |m| m.as_str()),
                category,
                optional_re.is_match(rest),
            ),
        );
    }
    Ok(deps)
}

/// `mix.lock` entries as `name -> version`; git and path entries carry no version.
pub fn parse_mix_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let hex_re = regex::Regex::new(r#"^\s*"([^"]+)":\s*\{:hex,\s*:[^,\s]+,\s*"([^"]+)""#)
        .map_err(DepcheckError::LockRegex)?;
    let git_re =
        regex::Regex::new(r#"^\s*"([^"]+)":\s*\{:git,"#).map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    for line in text.lines() {
        if let Some(caps) = hex_re.captures(line) {
            pkgs.insert(normalize_lower(&caps[1]), caps[2].to_owned());
        } else if let Some(caps) = git_re.captures(line) {
            pkgs.insert(normalize_lower(&caps[1]), "*".to_owned());
        }
    }
    Ok(pkgs)
}

/// The text of the bracketed list that follows `{deps,` and the rest of the file after it.
fn bracket_list(text: &str, from: usize) -> Option<&str> {
    let open = from + text[from..].find('[')?;
    let mut depth = 0usize;
    let mut in_string = false;
    for (at, ch) in text[open..].char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '[' | '{' if !in_string => depth += 1,
            ']' | '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open + 1..open + at]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits a list body into its top-level elements.
fn split_elements(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut start = 0;
    for (at, ch) in body.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '[' | '{' if !in_string => depth += 1,
            ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            ',' if !in_string && depth == 0 => {
                out.push(&body[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    out.push(&body[start..]);
    out.into_iter()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect()
}

pub fn parse_erlang_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let raw = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let text: String = raw
        .lines()
        .map(|line| line.split('%').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let head = regex::Regex::new(r"\{\s*deps\s*,").map_err(DepcheckError::ManifestRegex)?;
    let name_re = regex::Regex::new(r#"^\{?\s*([a-z][A-Za-z0-9_]*)\s*(?:,\s*"([^"]*)")?"#)
        .map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    for found in head.find_iter(&text) {
        let Some(body) = bracket_list(&text, found.end()) else {
            continue;
        };
        for element in split_elements(body) {
            let Some(caps) = name_re.captures(element) else {
                continue;
            };
            deps.entry(normalize_lower(&caps[1])).or_insert_with(|| {
                dep_info(
                    &caps[1],
                    caps.get(2).map_or("", |m| m.as_str()),
                    "prod",
                    false,
                )
            });
        }
    }
    Ok(deps)
}

/// `rebar.lock` entries as `name -> version`; git entries carry no version.
pub fn parse_rebar_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let pkg_re =
        regex::Regex::new(r#"\{<<"([^"]+)">>\s*,\s*\{pkg\s*,\s*<<"[^"]+">>\s*,\s*<<"([^"]+)">>"#)
            .map_err(DepcheckError::LockRegex)?;
    let git_re = regex::Regex::new(r#"\{<<"([^"]+)">>\s*,\s*\{git\s*,"#)
        .map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    for caps in pkg_re.captures_iter(&text) {
        pkgs.insert(normalize_lower(&caps[1]), caps[2].to_owned());
    }
    for caps in git_re.captures_iter(&text) {
        pkgs.insert(normalize_lower(&caps[1]), "*".to_owned());
    }
    Ok(pkgs)
}
