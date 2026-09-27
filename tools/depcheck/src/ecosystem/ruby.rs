use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

pub fn normalize_ruby(name: &str) -> String {
    name.to_lowercase()
}

pub fn parse_ruby_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let gem_re = regex::Regex::new(r#"(?m)^\s*gem\s+["']([^"']+)["']\s*(?:,\s*["']([^"']*)["'])?"#)
        .map_err(DepcheckError::ManifestRegex)?;
    let optional_re =
        regex::Regex::new(r"(?i)#\s*optional\b").map_err(DepcheckError::ManifestRegex)?;
    let platform_re =
        regex::Regex::new(r"(?i)#\s*platform\b").map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    for rawline in text.lines() {
        let line = rawline.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let low = line.to_lowercase();
        if low.starts_with("source ") || low.starts_with("ruby ") {
            continue;
        }
        let Some(caps) = gem_re.captures(line) else {
            continue;
        };
        let name = caps[1].to_owned();
        let ver = caps
            .get(2)
            .map(|m| m.as_str().to_owned())
            .unwrap_or_default();
        let spec = if ver.trim().is_empty() {
            "*".to_owned()
        } else {
            ver.trim().to_owned()
        };
        deps.insert(
            normalize_ruby(&name),
            DepInfo {
                spec,
                category: "prod".to_owned(),
                optional: optional_re.is_match(line),
                platform: platform_re.is_match(line),
                raw: name,
                peer: false,
                sha256: String::new(),
            },
        );
    }
    Ok(deps)
}

pub fn parse_ruby_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let re = regex::Regex::new(r"^    ([A-Za-z0-9_.\-]+) \(([^)]+)\)")
        .map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    for line in text.lines() {
        if let Some(caps) = re.captures(line) {
            pkgs.insert(normalize_ruby(&caps[1]), caps[2].trim().to_owned());
        }
    }
    Ok(pkgs)
}
