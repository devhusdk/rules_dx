use std::collections::BTreeMap;
use std::path::Path;

use crate::toml_util::{toml_bool, toml_string};
use crate::{DepInfo, DepcheckError};

pub fn normalize_jvm(name: &str) -> String {
    name.to_lowercase()
}

pub fn parse_jvm_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::ManifestToml)?;
    let mut deps = BTreeMap::new();
    let items = data
        .get("dep")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for item in items {
        let grp = item
            .get("group")
            .and_then(toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let art = item
            .get("artifact")
            .and_then(toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let ver = item
            .get("version")
            .and_then(toml_string)
            .unwrap_or_else(|| "*".to_owned())
            .trim()
            .to_owned();
        if grp.is_empty() || art.is_empty() {
            return Err(DepcheckError::JvmEntry);
        }
        let scope = item
            .get("scope")
            .and_then(toml_string)
            .unwrap_or_else(|| "compile".to_owned())
            .trim()
            .to_lowercase();
        let category = if scope == "test" || scope == "dev" {
            "dev"
        } else {
            "prod"
        };
        let key = normalize_jvm(&format!("{grp}:{art}"));
        deps.insert(
            key,
            DepInfo {
                spec: ver,
                category: category.to_owned(),
                optional: item.get("optional").map(toml_bool).unwrap_or(false),
                platform: item.get("platform").map(toml_bool).unwrap_or(false),
                raw: format!("{grp}:{art}"),
                peer: false,
                sha256: String::new(),
            },
        );
    }
    Ok(deps)
}

pub fn parse_jvm_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    let mut pkgs = BTreeMap::new();
    if let Some(arts) = data.get("artifacts").and_then(|v| v.as_object()) {
        for (coord, info) in arts {
            let ver = info
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            pkgs.insert(normalize_jvm(coord), ver);
        }
    }
    Ok(pkgs)
}

pub fn parse_maven_artifacts_list(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let re = regex::Regex::new(r#""([^":\s]+:[^":\s]+:[^":\s]+)""#)
        .map_err(DepcheckError::ManifestRegex)?;
    let mut deps = BTreeMap::new();
    for caps in re.captures_iter(&text) {
        let coord = caps[1].to_owned();
        let mut parts = coord.split(':');
        let (Some(group), Some(artifact), Some(version)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if parts.next().is_some() {
            continue;
        }
        deps.insert(
            normalize_jvm(&format!("{group}:{artifact}")),
            DepInfo {
                spec: version.to_owned(),
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: format!("{group}:{artifact}"),
                peer: false,
                sha256: String::new(),
            },
        );
    }
    if deps.is_empty() {
        return Err(DepcheckError::NoMavenCoords);
    }
    Ok(deps)
}
