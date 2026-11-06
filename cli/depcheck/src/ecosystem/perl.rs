use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Reads a `cpanfile`: `requires`, `recommends`, `suggests`, `test_requires`,
/// `build_requires` and `configure_requires`, also inside `on 'phase' => sub { ... }`.
pub fn parse_perl_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let on_re =
        regex::Regex::new(r#"^on\s*\(?\s*['"]?(\w+)"#).map_err(DepcheckError::ManifestRegex)?;
    let req_re = regex::Regex::new(
        r#"^(requires|recommends|suggests|test_requires|build_requires|configure_requires)\s*\(?\s*['"]([\w:]+)['"]\s*(?:(?:,|=>)\s*(?:['"]([^'"]*)['"]|([\w.]+)))?"#,
    )
    .map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    let mut phase = String::new();
    for rawline in text.lines() {
        let line = rawline.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(caps) = on_re.captures(line) {
            phase = caps[1].to_owned();
            continue;
        }
        if line.starts_with('}') {
            phase.clear();
            continue;
        }
        let Some(caps) = req_re.captures(line) else {
            continue;
        };
        let name = caps[2].to_owned();
        if name == "perl" {
            continue;
        }
        let version = caps
            .get(3)
            .or_else(|| caps.get(4))
            .map_or("", |m| m.as_str());
        let category = match (&caps[1], phase.as_str()) {
            ("test_requires", _) | (_, "test" | "develop" | "author") => "dev",
            ("build_requires" | "configure_requires", _) | (_, "build" | "configure") => "build",
            _ => "prod",
        };
        put(
            &mut deps,
            name.clone(),
            DepInfo {
                spec: perl_spec(version),
                category: category.to_owned(),
                optional: matches!(&caps[1], "recommends" | "suggests"),
                platform: false,
                raw: name,
                peer: false,
                sha256: String::new(),
                package: String::new(),
            },
        );
    }
    Ok(deps)
}

/// A bare Perl version means "at least"; `0` means any.
fn perl_spec(version: &str) -> String {
    let version = version.trim();
    let bare = version
        .strip_prefix('v')
        .unwrap_or(version)
        .starts_with(|c: char| c.is_ascii_digit());
    match version {
        "" | "0" => "*".to_owned(),
        _ if bare => format!(">= {version}"),
        _ => version.to_owned(),
    }
}

/// Maps every module a `cpanfile.snapshot` distribution provides to its version.
pub fn parse_perl_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let mut modules = BTreeMap::new();
    let mut provides = false;
    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        let item = line.trim();
        if indent <= 4 {
            provides = indent == 4 && item == "provides:";
        } else if provides && indent == 6 {
            if let Some((module, version)) = item.split_once(' ') {
                modules.insert(module.to_owned(), version.trim().to_owned());
            }
        }
    }
    Ok(modules)
}
