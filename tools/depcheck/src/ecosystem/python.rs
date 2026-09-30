use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use pep440_rs::{Version, VersionSpecifiers};

use crate::toml_util::parse_toml_file;
use crate::{DepInfo, DepcheckError};

const PLATFORM_MARKERS: [&str; 4] = ["sys_platform", "sys-platform", "platform_system", "os_name"];

pub fn normalize_py(name: &str) -> String {
    name.to_lowercase().replace(['-', '.'], "_")
}

struct Requirement {
    name: String,
    spec: String,
    platform: bool,
}

fn parse_requirement(item: &str) -> Option<Requirement> {
    let (head, marker) = match item.split_once(';') {
        Some((head, marker)) => (head.trim(), Some(marker)),
        None => (item.trim(), None),
    };
    let end = head
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')))
        .map_or(head.len(), |(idx, _)| idx);
    if end == 0 {
        return None;
    }
    let name = &head[..end];
    let after_extras = match head[end..].trim_start().strip_prefix('[') {
        Some(inner) => match inner.find(']') {
            Some(close) => &inner[close + 1..],
            None => "",
        },
        None => head[end..].trim(),
    };
    let spec = match VersionSpecifiers::from_str(after_extras.trim()) {
        Ok(specifiers) => {
            let text = specifiers.to_string();
            if text.is_empty() {
                "*".to_owned()
            } else {
                text
            }
        }
        Err(_) => "*".to_owned(),
    };
    let platform = marker.is_some_and(|marker| {
        PLATFORM_MARKERS
            .iter()
            .any(|marker_name| marker.contains(marker_name))
    });
    Some(Requirement {
        name: name.to_owned(),
        spec,
        platform,
    })
}

pub fn satisfies_py(spec: &str, locked: &str) -> Option<bool> {
    let specifiers = VersionSpecifiers::from_str(spec.trim()).ok()?;
    let version = Version::from_str(locked.trim()).ok()?;
    Some(specifiers.contains(&version))
}

pub fn parse_python_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let data = parse_toml_file(path)?;
    let mut deps = BTreeMap::new();
    if let Some(proj) = data.get("project").and_then(|v| v.as_table()) {
        if let Some(list) = proj.get("dependencies").and_then(|v| v.as_array()) {
            for item in list {
                let Some(text) = item.as_str() else { continue };
                let Some(requirement) = parse_requirement(text) else {
                    continue;
                };
                deps.insert(
                    normalize_py(&requirement.name),
                    DepInfo {
                        spec: requirement.spec,
                        category: "prod".to_owned(),
                        optional: false,
                        platform: requirement.platform,
                        raw: requirement.name,
                        peer: false,
                        sha256: String::new(),
                    },
                );
            }
        }
        if let Some(opt) = proj.get("optional-dependencies").and_then(|v| v.as_table()) {
            for items in opt.values() {
                let Some(list) = items.as_array() else {
                    continue;
                };
                for item in list {
                    let Some(text) = item.as_str() else { continue };
                    let Some(requirement) = parse_requirement(text) else {
                        continue;
                    };
                    deps.insert(
                        normalize_py(&requirement.name),
                        DepInfo {
                            spec: requirement.spec,
                            category: "dev".to_owned(),
                            optional: true,
                            platform: false,
                            raw: requirement.name,
                            peer: false,
                            sha256: String::new(),
                        },
                    );
                }
            }
        }
    }
    if let Some(groups) = data.get("dependency-groups").and_then(|v| v.as_table()) {
        for items in groups.values() {
            let Some(list) = items.as_array() else {
                continue;
            };
            for item in list {
                let Some(text) = item.as_str() else { continue };
                let Some(requirement) = parse_requirement(text) else {
                    continue;
                };
                deps.insert(
                    normalize_py(&requirement.name),
                    DepInfo {
                        spec: requirement.spec,
                        category: "dev".to_owned(),
                        optional: false,
                        platform: false,
                        raw: requirement.name,
                        peer: false,
                        sha256: String::new(),
                    },
                );
            }
        }
    }
    Ok(deps)
}

pub fn parse_python_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::LockToml)?;
    let mut pkgs = BTreeMap::new();
    if let Some(list) = data.get("package").and_then(|v| v.as_array()) {
        for item in list {
            let name = item
                .get("name")
                .and_then(crate::toml_util::toml_string)
                .unwrap_or_default();
            let ver = item
                .get("version")
                .and_then(crate::toml_util::toml_string)
                .unwrap_or_default();
            if !name.is_empty() {
                pkgs.insert(normalize_py(&name), ver);
            }
        }
    }
    if pkgs.is_empty() {
        let re = regex::Regex::new(r#"name\s*=\s*"([^"]+)"\s*\n\s*version\s*=\s*"([^"]+)""#)
            .map_err(DepcheckError::LockRegex)?;
        for caps in re.captures_iter(&text) {
            pkgs.insert(normalize_py(&caps[1]), caps[2].to_owned());
        }
    }
    Ok(pkgs)
}
