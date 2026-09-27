use std::collections::BTreeMap;
use std::path::Path;

use crate::toml_util::{toml_bool, toml_string};
use crate::{DepInfo, DepcheckError};

pub fn normalize_cc(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

pub fn parse_cc_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::ManifestToml)?;
    let mut deps = BTreeMap::new();
    let items = data
        .get("dep")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for item in items {
        let name = item
            .get("name")
            .and_then(toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let ver = item
            .get("version")
            .and_then(toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let sha = item
            .get("sha256")
            .and_then(toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if name.is_empty() {
            return Err(DepcheckError::CcEntry);
        }
        let scope = item
            .get("scope")
            .and_then(toml_string)
            .unwrap_or_else(|| "prod".to_owned())
            .trim()
            .to_lowercase();
        let category = if scope == "test" || scope == "dev" {
            "dev"
        } else {
            "prod"
        };
        deps.insert(
            normalize_cc(&name),
            DepInfo {
                spec: if ver.is_empty() { "*".to_owned() } else { ver },
                category: category.to_owned(),
                optional: item.get("optional").map(toml_bool).unwrap_or(false),
                platform: item.get("platform").map(toml_bool).unwrap_or(false),
                sha256: sha,
                raw: name,
                peer: false,
            },
        );
    }
    Ok(deps)
}

pub fn parse_cc_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    let mut pkgs = BTreeMap::new();
    if let Some(map) = data.get("packages").and_then(|v| v.as_object()) {
        for (name, info) in map {
            let ver = info
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            pkgs.insert(normalize_cc(name), ver);
        }
    }
    Ok(pkgs)
}

pub fn parse_cc_lock_sha(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(data): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    if let Some(map) = data.get("packages").and_then(|v| v.as_object()) {
        for (name, info) in map {
            let sha = info
                .get("sha256")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            out.insert(normalize_cc(name), sha);
        }
    }
    out
}
