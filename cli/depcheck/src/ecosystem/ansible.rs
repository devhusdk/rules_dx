use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

fn manifest_error(message: String) -> DepcheckError {
    DepcheckError::ManifestParse(message)
}

/// Reads the `collections` of an ansible `requirements.yml`: each entry's `name` and `version`.
pub fn parse_ansible_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let root: yaml_serde::Value = yaml_serde::from_str(&text)
        .map_err(|error| manifest_error(format!("invalid requirements.yml: {error}")))?;
    let entries = match root.get("collections") {
        None | Some(yaml_serde::Value::Null) => return Ok(BTreeMap::new()),
        Some(yaml_serde::Value::Sequence(entries)) => entries,
        Some(_) => return Err(manifest_error("collections must be a list".to_owned())),
    };
    let mut deps = BTreeMap::new();
    for entry in entries {
        let (name, version) = match entry {
            yaml_serde::Value::String(name) => (name.as_str(), None),
            yaml_serde::Value::Mapping(_) => (
                entry
                    .get("name")
                    .and_then(yaml_serde::Value::as_str)
                    .ok_or_else(|| manifest_error("a collection entry has no name".to_owned()))?,
                entry.get("version").and_then(yaml_serde::Value::as_str),
            ),
            yaml_serde::Value::Null
            | yaml_serde::Value::Bool(_)
            | yaml_serde::Value::Number(_)
            | yaml_serde::Value::Sequence(_)
            | yaml_serde::Value::Tagged(_) => {
                return Err(manifest_error(
                    "a collection entry is neither a name nor a mapping".to_owned(),
                ));
            }
        };
        let spec = version.map_or("", str::trim);
        put(
            &mut deps,
            name.to_owned(),
            DepInfo {
                spec: if spec.is_empty() { "*" } else { spec }.to_owned(),
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: name.to_owned(),
                peer: false,
                sha256: String::new(),
                package: String::new(),
            },
        );
    }
    Ok(deps)
}

/// Maps every collection in an `ansible.lock.json` to its version.
pub fn parse_ansible_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    Ok(super::lock_packages(path)?
        .into_iter()
        .filter_map(|(name, version)| Some((name, version?)))
        .collect())
}
