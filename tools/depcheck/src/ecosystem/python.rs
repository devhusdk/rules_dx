use std::collections::BTreeMap;
use std::path::Path;

use crate::toml_util::parse_toml_file;
use crate::{DepInfo, DepcheckError};

pub fn normalize_py(name: &str) -> String {
    name.to_lowercase().replace(['-', '.'], "_")
}

fn python_spec_from_rest(rest: &str) -> String {
    let mut spec = String::new();
    for ch in rest.chars() {
        if matches!(
            ch,
            '=' | '<' | '>' | '^' | '~' | '!' | '.' | ',' | '*' | ' ' | '\t'
        ) || ch.is_ascii_digit()
        {
            spec.push(ch);
        } else {
            break;
        }
    }
    let trimmed = spec.trim().to_owned();
    if trimmed.is_empty() {
        "*".to_owned()
    } else {
        trimmed
    }
}

fn python_raw_name(item: &str) -> Option<(String, String)> {
    let item = item.trim();
    if item.is_empty() {
        return None;
    }
    let mut end = 0usize;
    for (idx, ch) in item.char_indices() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '[' | ']') {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let mut head = item[..end].to_owned();
    if let Some(bracket) = head.find('[') {
        head = head[..bracket].to_owned();
    }
    if head.is_empty() {
        return None;
    }
    let rest = item[end..].trim().to_owned();
    Some((head, rest))
}

pub fn parse_python_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let data = parse_toml_file(path)?;
    let mut deps = BTreeMap::new();
    if let Some(proj) = data.get("project").and_then(|v| v.as_table()) {
        if let Some(list) = proj.get("dependencies").and_then(|v| v.as_array()) {
            for item in list {
                let Some(text) = item.as_str() else { continue };
                let Some((rawname, rest)) = python_raw_name(text) else {
                    continue;
                };
                let marker_platform = rest.contains("sys_platform")
                    || rest.contains("sys-platform")
                    || rest.contains("platform_system")
                    || rest.contains("os_name");
                let spec = python_spec_from_rest(&rest);
                deps.insert(
                    normalize_py(&rawname),
                    DepInfo {
                        spec,
                        category: "prod".to_owned(),
                        optional: false,
                        platform: marker_platform,
                        raw: rawname,
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
                    let Some((rawname, rest)) = python_raw_name(text) else {
                        continue;
                    };
                    let spec = python_spec_from_rest(&rest);
                    deps.insert(
                        normalize_py(&rawname),
                        DepInfo {
                            spec,
                            category: "dev".to_owned(),
                            optional: true,
                            platform: false,
                            raw: rawname,
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
                let Some((rawname, rest)) = python_raw_name(text) else {
                    continue;
                };
                let spec = python_spec_from_rest(&rest);
                deps.insert(
                    normalize_py(&rawname),
                    DepInfo {
                        spec,
                        category: "dev".to_owned(),
                        optional: false,
                        platform: false,
                        raw: rawname,
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
