use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

pub fn normalize_js(name: &str) -> String {
    name.to_lowercase()
}

const CONCRETE_FIELDS: [(&str, &str, bool); 3] = [
    ("dependencies", "prod", false),
    ("optionalDependencies", "prod", true),
    ("devDependencies", "dev", false),
];

fn manifest_section<'a>(
    data: &'a serde_json::Value,
    field: &str,
) -> Result<Option<&'a serde_json::Map<String, serde_json::Value>>, DepcheckError> {
    match data.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(DepcheckError::ManifestParse(format!(
            "{field} must be an object"
        ))),
    }
}

fn manifest_spec(
    field: &str,
    name: &str,
    spec: &serde_json::Value,
) -> Result<String, DepcheckError> {
    spec.as_str()
        .map(str::to_owned)
        .ok_or_else(|| DepcheckError::ManifestParse(format!("{field}.{name} must be a string")))
}

pub fn parse_js_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(DepcheckError::ManifestJson)?;
    let mut deps: BTreeMap<String, DepInfo> = BTreeMap::new();
    for (field, category, optional) in CONCRETE_FIELDS {
        let Some(map) = manifest_section(&data, field)? else {
            continue;
        };
        for (name, spec) in map {
            let spec = manifest_spec(field, name, spec)?;
            let key = normalize_js(name);
            if let Some(prev) = deps.get(&key) {
                if prev.spec != spec {
                    return Err(DepcheckError::ManifestParse(format!(
                        "{name} is declared with conflicting requirements: {} and {spec}",
                        prev.spec
                    )));
                }
                continue;
            }
            deps.insert(
                key,
                DepInfo {
                    spec,
                    category: category.to_owned(),
                    optional,
                    platform: optional,
                    raw: name.clone(),
                    peer: false,
                    sha256: String::new(),
                },
            );
        }
    }
    if let Some(map) = manifest_section(&data, "peerDependencies")? {
        for (name, spec) in map {
            let spec = manifest_spec("peerDependencies", name, spec)?;
            deps.entry(normalize_js(name)).or_insert_with(|| DepInfo {
                spec,
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: name.clone(),
                peer: true,
                sha256: String::new(),
            });
        }
    }
    Ok(deps)
}

pub fn parse_pnpm_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let section_re = regex::Regex::new(r"^\S+:\s*$").map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    let mut in_packages = false;
    for line in text.lines() {
        if line.starts_with("packages:") && line.trim() == "packages:" {
            in_packages = true;
            continue;
        }
        if in_packages
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && line.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && !line.trim_start().starts_with('\'')
            && !line.trim_start().starts_with('"')
            && section_re.is_match(line)
        {
            in_packages = false;
            continue;
        }
        if !in_packages {
            continue;
        }
        let trimmed = line.trim().trim_matches(|c| c == '\'' || c == '"');
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let raw = trimmed[..colon]
            .trim()
            .trim_matches(|c| c == '\'' || c == '"' || c == ' ');
        let Some(at) = raw.rfind('@') else {
            continue;
        };
        if at == 0 {
            continue;
        }
        let name = raw[..at]
            .trim()
            .trim_matches(|c| c == '\'' || c == '"' || c == ' ');
        let mut ver = raw[at + 1..]
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_owned();
        if let Some(paren) = ver.find('(') {
            ver = ver[..paren].to_owned();
        }
        ver = ver
            .trim_matches(|c| c == '\'' || c == '"' || c == ' ' || c == '(' || c == ')')
            .to_owned();
        if name.is_empty() || ver.is_empty() {
            continue;
        }
        pkgs.insert(normalize_js(name), ver);
    }
    Ok(pkgs)
}
