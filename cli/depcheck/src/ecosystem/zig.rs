use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

/// Reads the `.dependencies` of a manifest. A dependency's `.hash` is its lock, so it
/// lands in `sha256`; `.lazy = true` marks it optional. Path dependencies have no hash and are
/// skipped.
pub fn parse_zig_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let text = strip_comments(&text);
    let entry_re = regex::Regex::new(r#"\.(?:@"([^"]+)"|(\w+))\s*=\s*\.\{"#)
        .map_err(DepcheckError::ManifestRegex)?;
    let hash_re =
        regex::Regex::new(r#"\.hash\s*=\s*"([^"]*)""#).map_err(DepcheckError::ManifestRegex)?;
    let lazy_re =
        regex::Regex::new(r"\.lazy\s*=\s*true\b").map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    let Some(block) = dependencies_block(&text) else {
        return Ok(deps);
    };
    let mut at = 0;
    while let Some(caps) = block.get(at..).and_then(|tail| entry_re.captures(tail)) {
        let whole = caps.get(0).map_or(0, |m| m.end());
        let body_start = at + whole;
        let rest = block.get(body_start..).unwrap_or_default();
        let body_len = matching_brace(rest);
        let body = rest.get(..body_len).unwrap_or(rest);
        at = body_start + body_len;
        if body.contains(".path") {
            continue;
        }
        let name = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map_or("", |m| m.as_str())
            .to_owned();
        deps.insert(
            name.clone(),
            DepInfo {
                spec: "*".to_owned(),
                category: "prod".to_owned(),
                optional: lazy_re.is_match(body),
                platform: false,
                raw: name,
                peer: false,
                sha256: hash_re
                    .captures(body)
                    .map_or(String::new(), |hash| hash[1].to_owned()),
                package: String::new(),
            },
        );
    }
    Ok(deps)
}

/// Reads the fetched package hashes, one `<name> <hash>` per line. `#` starts a comment.
pub fn parse_zig_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let mut hashes = BTreeMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((name, hash)) = line.split_once(char::is_whitespace) {
            hashes.insert(name.to_owned(), hash.trim().to_owned());
        }
    }
    Ok(hashes)
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut quoted = false;
            let mut prev = ' ';
            for (at, ch) in line.char_indices() {
                if ch == '"' && prev != '\\' {
                    quoted = !quoted;
                }
                if !quoted && ch == '/' && prev == '/' {
                    return line.get(..at - 1).unwrap_or(line);
                }
                prev = ch;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text between the braces of `.dependencies = .{ ... }`.
fn dependencies_block(text: &str) -> Option<&str> {
    let re = regex::Regex::new(r"\.dependencies\s*=\s*\.\{").ok()?;
    let start = re.find(text)?.end();
    let rest = text.get(start..)?;
    rest.get(..matching_brace(rest))
}

/// Offset of the `}` closing an already opened brace, or the text length when unclosed.
fn matching_brace(text: &str) -> usize {
    let mut depth = 1usize;
    let mut quoted = false;
    let mut prev = ' ';
    for (at, ch) in text.char_indices() {
        if ch == '"' && prev != '\\' {
            quoted = !quoted;
        } else if !quoted && ch == '{' {
            depth += 1;
        } else if !quoted && ch == '}' {
            depth -= 1;
            if depth == 0 {
                return at;
            }
        }
        prev = ch;
    }
    text.len()
}
