use std::collections::BTreeMap;
use std::path::Path;

use dx_ecosystems::Ecosystem;

use crate::toml_util::{toml_bool, toml_string};
use crate::{DepInfo, DepcheckError};

use super::normalize_lower;

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
        let key = normalize_lower(&format!("{grp}:{art}"));
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
                package: String::new(),
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
            pkgs.insert(normalize_lower(coord), ver);
        }
    }
    Ok(pkgs)
}

fn maven_lock_label() -> String {
    let lock = Ecosystem::Maven
        .registry_names()
        .into_iter()
        .find(|name| name.ends_with(".json"))
        .unwrap_or_default();
    format!("\"//third_party/jvm:{lock}\"")
}

fn maven_install_artifacts(text: &str) -> Option<&str> {
    let label = maven_lock_label();
    let mut rest = text;
    while let Some(at) = rest.find("maven.install(") {
        let body = &rest[at + "maven.install(".len()..];
        let mut depth = 1usize;
        let mut end = body.len();
        for (i, c) in body.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let call = &body[..end];
        if call.contains(&label) {
            let list = &call[call.find("artifacts")?..];
            let open = list.find('[')?;
            let close = open + list[open..].find(']')?;
            return Some(&list[open..close]);
        }
        rest = &body[end..];
    }
    None
}

/// `group:artifact:version` of every `[libraries]` entry in a version catalog.
fn catalog_coords(text: &str) -> Result<Vec<String>, DepcheckError> {
    let data: toml::Value = toml::from_str(text).map_err(DepcheckError::ManifestToml)?;
    let versions = data.get("versions").and_then(toml::Value::as_table);
    let Some(libraries) = data.get("libraries").and_then(toml::Value::as_table) else {
        return Ok(Vec::new());
    };
    let mut coords = Vec::new();
    for entry in libraries.values() {
        if let Some(coord) = entry.as_str() {
            coords.push(coord.to_owned());
            continue;
        }
        let module = entry
            .get("module")
            .and_then(toml::Value::as_str)
            .map_or_else(
                || {
                    let group = entry.get("group").and_then(toml::Value::as_str)?;
                    let name = entry.get("name").and_then(toml::Value::as_str)?;
                    Some(format!("{group}:{name}"))
                },
                |module| Some(module.to_owned()),
            );
        let version = match entry.get("version") {
            Some(toml::Value::String(version)) => Some(version.clone()),
            Some(toml::Value::Table(table)) => table
                .get("ref")
                .and_then(toml::Value::as_str)
                .and_then(|name| versions?.get(name)?.as_str())
                .map(str::to_owned),
            _ => None,
        };
        if let (Some(module), Some(version)) = (module, version) {
            coords.push(format!("{module}:{version}"));
        }
    }
    Ok(coords)
}

pub fn parse_maven_artifacts_list(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let coords = if path.extension().is_some_and(|ext| ext == "toml") {
        catalog_coords(&text)?
    } else {
        let list = maven_install_artifacts(&text).ok_or(DepcheckError::NoMavenCoords)?;
        let re = regex::Regex::new(r#""([^":\s]+:[^":\s]+:[^":\s]+)""#)
            .map_err(DepcheckError::ManifestRegex)?;
        re.captures_iter(list)
            .map(|caps| caps[1].to_owned())
            .collect()
    };
    let mut deps = BTreeMap::new();
    for coord in coords {
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
            normalize_lower(&format!("{group}:{artifact}")),
            DepInfo {
                spec: version.to_owned(),
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: format!("{group}:{artifact}"),
                peer: false,
                sha256: String::new(),
                package: String::new(),
            },
        );
    }
    if deps.is_empty() {
        return Err(DepcheckError::NoMavenCoords);
    }
    Ok(deps)
}
