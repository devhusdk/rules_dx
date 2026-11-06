use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::{normalize_lower, put};

/// Reads `[dependencies]` and `[dev-dependencies]` of an `fpm.toml`.
pub fn parse_fortran_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let data = crate::toml_util::parse_toml_file(path)?;
    let mut deps = BTreeMap::new();
    for (table, category) in [("dependencies", "prod"), ("dev-dependencies", "dev")] {
        let Some(entries) = data.get(table).and_then(|v| v.as_table()) else {
            continue;
        };
        for (name, value) in entries {
            let spec = value
                .as_str()
                .or_else(|| value.get("version").and_then(|v| v.as_str()))
                .filter(|text| !text.trim().is_empty())
                .unwrap_or("*");
            put(
                &mut deps,
                normalize_lower(name),
                DepInfo {
                    spec: spec.trim().to_owned(),
                    category: category.to_owned(),
                    optional: false,
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

/// Reads an `fpm.lock`: one `[dependencies.<name>]` table per resolved package, with its `version`.
pub fn parse_fortran_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::LockToml)?;
    let mut pkgs = BTreeMap::new();
    for (name, value) in data
        .get("dependencies")
        .and_then(|v| v.as_table())
        .into_iter()
        .flatten()
    {
        let version = value.get("version").and_then(|v| v.as_str()).unwrap_or("0");
        pkgs.insert(normalize_lower(name), version.to_owned());
    }
    Ok(pkgs)
}
