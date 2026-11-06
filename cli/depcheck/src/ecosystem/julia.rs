use std::collections::BTreeMap;
use std::path::Path;

use crate::toml_util::parse_toml_file;
use crate::{DepInfo, DepcheckError};

use super::put;

/// Reads `[deps]`, `[weakdeps]` and `[extras]` of a `Project.toml`. Extras are test-only and
/// weak dependencies are optional.
pub fn parse_julia_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let data = parse_toml_file(path)?;
    let mut deps = BTreeMap::new();
    for (category, optional, key) in [
        ("prod", false, "deps"),
        ("prod", true, "weakdeps"),
        ("dev", false, "extras"),
    ] {
        let Some(table) = data.get(key).and_then(|v| v.as_table()) else {
            continue;
        };
        for name in table.keys() {
            if deps.contains_key(name) {
                continue;
            }
            put(
                &mut deps,
                name.clone(),
                DepInfo {
                    spec: "*".to_owned(),
                    category: category.to_owned(),
                    optional,
                    platform: false,
                    raw: name.clone(),
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
    }
    Ok(deps)
}

/// Maps every package in a `Manifest.toml` (format 1 or 2) to its version. Standard libraries
/// carry no version and map to `stdlib`.
pub fn parse_julia_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let data = parse_toml_file(path)?;
    let mut locked = BTreeMap::new();
    let Some(packages) = data
        .get("deps")
        .and_then(|v| v.as_table())
        .or_else(|| data.as_table())
    else {
        return Ok(locked);
    };
    for (name, entries) in packages {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        let version = entries
            .iter()
            .find_map(|entry| entry.get("version").and_then(|v| v.as_str()))
            .unwrap_or("stdlib");
        locked.insert(name.clone(), version.to_owned());
    }
    Ok(locked)
}
