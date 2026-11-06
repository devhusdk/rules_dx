use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Reads the `dependencies`, `build_dependencies` and `test_dependencies` tables of a
/// `*.rockspec`. The `lua` entry only pins the interpreter and is skipped.
pub fn parse_lua_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let text = strip_comments(&text);
    let table_re = regex::Regex::new(
        r"\b(dependencies|build_dependencies|test_dependencies)\s*=\s*\{([^}]*)\}",
    )
    .map_err(DepcheckError::ManifestRegex)?;
    let entry_re =
        regex::Regex::new(r#"["']([^"']+)["']"#).map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    for table in table_re.captures_iter(&text) {
        let category = match &table[1] {
            "build_dependencies" => "build",
            "test_dependencies" => "dev",
            _ => "prod",
        };
        for entry in entry_re.captures_iter(&table[2]) {
            let entry = entry[1].trim();
            let (name, spec) = entry
                .split_once(char::is_whitespace)
                .map_or((entry, ""), |(name, spec)| (name, spec.trim()));
            if name == "lua" || name.is_empty() {
                continue;
            }
            put(
                &mut deps,
                name.to_owned(),
                DepInfo {
                    spec: if spec.is_empty() { "*" } else { spec }.to_owned(),
                    category: category.to_owned(),
                    optional: false,
                    platform: false,
                    raw: name.to_owned(),
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
    }
    Ok(deps)
}

/// Reads a `luarocks.lock`: `["name"] = "version-revision"` pairs. The rock revision is dropped.
pub fn parse_lua_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let text = strip_comments(&text);
    let pair_re = regex::Regex::new(r#"\[\s*["']([^"']+)["']\s*\]\s*=\s*["']([^"']+)["']"#)
        .map_err(DepcheckError::LockRegex)?;
    let mut rocks = BTreeMap::new();
    for pair in pair_re.captures_iter(&text) {
        let version = pair[2]
            .rsplit_once('-')
            .map_or(&pair[2], |(version, _)| version);
        rocks.insert(pair[1].to_owned(), version.to_owned());
    }
    Ok(rocks)
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}
