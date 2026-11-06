use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError, LockVersions};

use super::{normalize_lower, push_version, put};

fn manifest_error(message: String) -> DepcheckError {
    DepcheckError::ManifestParse(message)
}

fn constraint(value: &yaml_serde::Value) -> String {
    let text = match value {
        yaml_serde::Value::String(text) => text.trim(),
        yaml_serde::Value::Mapping(map) => map
            .get("version")
            .and_then(yaml_serde::Value::as_str)
            .map_or("", str::trim),
        _ => "",
    };
    if text.is_empty() || text == "any" {
        "*".to_owned()
    } else {
        text.to_owned()
    }
}

/// `dependencies` and `dev_dependencies` of a pubspec.yaml.
pub fn parse_dart_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let root: yaml_serde::Value = yaml_serde::from_str(&text)
        .map_err(|error| manifest_error(format!("invalid pubspec.yaml: {error}")))?;
    let mut deps = BTreeMap::new();
    for (section, category) in [("dependencies", "prod"), ("dev_dependencies", "dev")] {
        let table = match root.get(section) {
            None | Some(yaml_serde::Value::Null) => continue,
            Some(yaml_serde::Value::Mapping(table)) => table,
            Some(_) => return Err(manifest_error(format!("{section} must be a mapping"))),
        };
        for (name, value) in table {
            let Some(name) = name.as_str() else {
                return Err(manifest_error(format!("{section} has a non-string name")));
            };
            put(
                &mut deps,
                normalize_lower(name),
                DepInfo {
                    spec: constraint(value),
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

fn lock_entries(path: &Path) -> Result<Vec<(String, String, String)>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let root: yaml_serde::Value = yaml_serde::from_str(&text)
        .map_err(|error| DepcheckError::LockParse(format!("invalid pubspec.lock: {error}")))?;
    let packages = root
        .get("packages")
        .and_then(yaml_serde::Value::as_mapping)
        .ok_or_else(|| DepcheckError::LockParse("pubspec.lock has no packages".to_owned()))?;
    let mut out = Vec::new();
    for (name, detail) in packages {
        let field = |key: &str| {
            detail
                .get(key)
                .and_then(yaml_serde::Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        let Some(name) = name.as_str() else {
            return Err(DepcheckError::LockParse(
                "pubspec.lock package name is not a string".to_owned(),
            ));
        };
        out.push((normalize_lower(name), field("version"), field("source")));
    }
    Ok(out)
}

/// `packages.<name>.version` of a pubspec.lock, SDK packages included.
pub fn parse_dart_lock(path: &Path) -> Result<LockVersions, DepcheckError> {
    let mut pkgs = LockVersions::new();
    for (name, version, _) in lock_entries(path)? {
        push_version(&mut pkgs, name, version);
    }
    Ok(pkgs)
}

/// Locked packages a source can import without the manifest naming them: hosted and git ones.
pub fn dart_lock_candidates(path: &Path) -> Result<Vec<String>, DepcheckError> {
    Ok(lock_entries(path)?
        .into_iter()
        .filter(|(_, _, source)| source == "hosted" || source == "git")
        .map(|(name, _, _)| name)
        .collect())
}
