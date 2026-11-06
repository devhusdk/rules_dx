use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::{normalize_lower, put};

/// Reads `composer.json` `require` and `require-dev`, skipping `php`, `ext-*` and `lib-*`.
pub fn parse_php_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(DepcheckError::ManifestJson)?;
    let mut deps = BTreeMap::new();
    for (field, category) in [("require", "prod"), ("require-dev", "dev")] {
        let Some(section) = data.get(field) else {
            continue;
        };
        let map = section
            .as_object()
            .ok_or_else(|| DepcheckError::ManifestParse(format!("{field} must be an object")))?;
        for (name, spec) in map {
            if name == "php" || name.starts_with("ext-") || name.starts_with("lib-") {
                continue;
            }
            let spec = spec.as_str().ok_or_else(|| {
                DepcheckError::ManifestParse(format!("{field}.{name} must be a string"))
            })?;
            put(
                &mut deps,
                normalize_lower(name),
                DepInfo {
                    spec: spec.to_owned(),
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

/// Maps every package in `composer.lock` (`packages` and `packages-dev`) to its version.
pub fn parse_php_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    let mut pkgs = BTreeMap::new();
    for field in ["packages", "packages-dev"] {
        let Some(list) = data.get(field).and_then(|v| v.as_array()) else {
            continue;
        };
        for pkg in list {
            let (Some(name), Some(version)) = (
                pkg.get("name").and_then(|v| v.as_str()),
                pkg.get("version").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let version = version.strip_prefix('v').unwrap_or(version);
            pkgs.insert(normalize_lower(name), version.to_owned());
        }
    }
    Ok(pkgs)
}
