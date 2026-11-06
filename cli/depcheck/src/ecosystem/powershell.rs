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
    let rest = &text[start.end()..];
    let chars: Vec<(usize, char)> = rest.char_indices().collect();
    let mut at = 0;
    let mut depth = 1usize;
    while at < chars.len() && depth > 0 {
        let (pos, ch) = chars[at];
        let mut entry: Option<(String, String)> = None;
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' | '"' => {
                let end = rest[pos + 1..].find(ch).map_or(rest.len(), |n| pos + 1 + n);
                entry = Some((rest[pos + 1..end].to_owned(), String::new()));
                at = chars.partition_point(|(p, _)| *p < end);
            }
            '@' if rest[pos..].starts_with("@{") => {
                let end = pos + 2 + brace_end(&rest[pos + 2..]);
                let body = &rest[pos + 2..end];
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

/// Reads the installed modules, one `<name> <version>` per line (`Get-InstalledModule`
/// output) under the lowercase name, as the manifest keys are. `#` starts a comment.
pub fn parse_powershell_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let mut modules = BTreeMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((name, version)) = line.split_once(char::is_whitespace) {
            modules.insert(name.to_lowercase(), version.trim().to_owned());
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
                    (None, '#') => return &line[..at],
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
