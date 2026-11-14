use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Reads `RequiredModules` of a `*.psd1`: bare names or `@{ ModuleName = ...; ModuleVersion = ... }`.
/// `ModuleVersion` is a minimum, `RequiredVersion` an exact pin, `MaximumVersion` a ceiling.
pub fn parse_powershell_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let text = strip_comments(&text);
    let start_re = regex::Regex::new(r"(?i)\bRequiredModules\s*=\s*@?\(")
        .map_err(DepcheckError::ManifestRegex)?;
    let field = |name: &str| {
        regex::Regex::new(&format!(r#"(?i)\b{name}\s*=\s*['"]([^'"]*)['"]"#))
            .map_err(DepcheckError::ManifestRegex)
    };
    let name_re = field("ModuleName")?;
    let min_re = field("ModuleVersion")?;
    let exact_re = field("RequiredVersion")?;
    let max_re = field("MaximumVersion")?;
    let mut deps = BTreeMap::new();
    let Some(start) = start_re.find(&text) else {
        return Ok(deps);
    };
    let rest = text.get(start.end()..).unwrap_or_default();
    let chars: Vec<(usize, char)> = rest.char_indices().collect();
    let mut at = 0;
    let mut depth = 1usize;
    while depth > 0 {
        let Some(&(pos, ch)) = chars.get(at) else {
            break;
        };
        let mut entry: Option<(String, String)> = None;
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' | '"' => {
                let quoted = rest.get(pos + 1..).unwrap_or_default();
                let value = quoted.split_once(ch).map_or(quoted, |(value, _)| value);
                let end = pos + 1 + value.len();
                entry = Some((value.to_owned(), String::new()));
                at = chars.partition_point(|(p, _)| *p < end);
            }
            '@' if rest.get(pos..).is_some_and(|tail| tail.starts_with("@{")) => {
                let inner = rest.get(pos + 2..).unwrap_or_default();
                let body_len = brace_end(inner);
                let end = pos + 2 + body_len;
                let body = inner.get(..body_len).unwrap_or_default();
                let get = |re: &regex::Regex| re.captures(body).map(|c| c[1].to_owned());
                let mut bounds = Vec::new();
                if let Some(version) = get(&exact_re) {
                    bounds.push(format!(">= {version}"));
                    bounds.push(format!("<= {version}"));
                } else if let Some(version) = get(&min_re) {
                    bounds.push(format!(">= {version}"));
                }
                if let (Some(version), false) = (get(&max_re), get(&exact_re).is_some()) {
                    bounds.push(format!("<= {version}"));
                }
                if let Some(name) = get(&name_re) {
                    entry = Some((name, bounds.join(", ")));
                }
                at = chars.partition_point(|(p, _)| *p < end);
            }
            _ => {}
        }
        if let Some((name, spec)) = entry {
            put(
                &mut deps,
                name.to_lowercase(),
                DepInfo {
                    spec: if spec.is_empty() {
                        "*".to_owned()
                    } else {
                        spec
                    },
                    category: "prod".to_owned(),
                    optional: false,
                    platform: false,
                    raw: name,
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
        at += 1;
    }
    Ok(deps)
}

/// Reads the lock: the name and version of each package under the lowercase name, as the
/// manifest keys are.
pub fn parse_powershell_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        DepcheckError::LockIo(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    let mut modules = BTreeMap::new();
    for package in json
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(name), Some(version)) = (package["name"].as_str(), package["version"].as_str())
        {
            modules.insert(name.to_lowercase(), version.to_owned());
        }
    }
    Ok(modules)
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut quote = None;
            for (at, ch) in line.char_indices() {
                match (quote, ch) {
                    (None, '\'' | '"') => quote = Some(ch),
                    (Some(open), _) if open == ch => quote = None,
                    (None, '#') => return line.get(..at).unwrap_or(line),
                    _ => {}
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Offset of the `}` closing an already opened brace, or the text length when unclosed.
fn brace_end(text: &str) -> usize {
    let mut depth = 1usize;
    for (at, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return at;
                }
            }
            _ => {}
        }
    }
    text.len()
}
