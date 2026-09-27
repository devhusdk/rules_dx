use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

pub fn normalize_go(name: &str) -> String {
    name.to_lowercase()
}

pub fn parse_go_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let mut deps = BTreeMap::new();
    let mut in_require = false;
    let single_re = regex::Regex::new(r"^require\s+(\S+)\s+(\S+)(.*)$")
        .map_err(DepcheckError::ManifestRegex)?;
    let block_re =
        regex::Regex::new(r"^(\S+)\s+(\S+)(.*)$").map_err(DepcheckError::ManifestRegex)?;
    let optional_re =
        regex::Regex::new(r"(?i)//\s*optional\b").map_err(DepcheckError::ManifestRegex)?;
    let platform_re =
        regex::Regex::new(r"(?i)//\s*platform\b").map_err(DepcheckError::ManifestRegex)?;
    let test_re = regex::Regex::new(r"(?i)//\s*test\b").map_err(DepcheckError::ManifestRegex)?;
    for rawline in text.lines() {
        let line = rawline.trim().to_owned();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("//") && !line.starts_with("// depcheck") {
            if line.starts_with("module ") || line.starts_with("go ") {
                continue;
            }
            if line.starts_with("//") {
                continue;
            }
        }
        if line.starts_with("require (") {
            in_require = true;
            continue;
        }
        if in_require && line == ")" {
            in_require = false;
            continue;
        }
        let caps = if line.starts_with("require ") && !in_require {
            single_re.captures(&line)
        } else if in_require {
            block_re.captures(&line)
        } else {
            None
        };
        let Some(caps) = caps else { continue };
        let module = caps[1].to_owned();
        let ver = caps[2].to_owned();
        let rest = caps
            .get(3)
            .map(|m| m.as_str().to_owned())
            .unwrap_or_default();
        let mut category = "prod".to_owned();
        let mut optional = false;
        let mut platform = false;
        let low = rest.to_lowercase();
        if low.contains("depcheck:test") {
            category = "dev".to_owned();
        }
        if low.contains("depcheck:optional") {
            optional = true;
        }
        if low.contains("depcheck:platform") {
            platform = true;
        }
        if optional_re.is_match(&rest) {
            optional = true;
        }
        if platform_re.is_match(&rest) {
            platform = true;
        }
        if test_re.is_match(&rest) {
            category = "dev".to_owned();
        }
        deps.insert(
            normalize_go(&module),
            DepInfo {
                spec: ver,
                category,
                optional,
                platform,
                raw: module,
                peer: false,
                sha256: String::new(),
            },
        );
    }
    Ok(deps)
}

pub fn parse_go_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let mut pkgs = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let module = parts[0].to_owned();
        let mut ver = parts[1].to_owned();
        if let Some(stripped) = ver.strip_suffix("/go.mod") {
            ver = stripped.to_owned();
        }
        let key = normalize_go(&module);
        pkgs.entry(key).or_insert(ver);
    }
    Ok(pkgs)
}
