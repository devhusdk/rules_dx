use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::{normalize_lower, put};

/// Reads `[dependencies]` and `[dev-dependencies]` of an fpm manifest.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or does not parse.
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

/// Reads an fpm lock: the `name` and `version` of each locked package.
///
/// # Errors
///
/// Returns an error when the lock cannot be read or does not parse.
pub fn parse_fortran_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    Ok(super::lock_packages(path)?
        .into_iter()
        .map(|(name, version)| {
            (
                normalize_lower(&name),
                version.unwrap_or_else(|| "0".to_owned()),
            )
        })
        .collect())
}
