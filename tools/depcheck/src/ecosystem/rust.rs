use std::collections::BTreeMap;
use std::path::Path;

use crate::toml_util::{parse_toml_file, toml_bool, toml_string};
use crate::{DepInfo, DepcheckError};

pub fn normalize_rs(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

pub fn parse_rust_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let data = parse_toml_file(path)?;
    let mut deps = BTreeMap::new();
    for (cat, key) in [
        ("prod", "dependencies"),
        ("dev", "dev-dependencies"),
        ("build", "build-dependencies"),
    ] {
        if let Some(tbl) = data.get(key).and_then(|v| v.as_table()) {
            for (name, val) in tbl {
                let (spec, optional) = match val {
                    toml::Value::String(s) => (s.clone(), false),
                    toml::Value::Table(t) => {
                        let spec = t
                            .get("version")
                            .and_then(toml_string)
                            .unwrap_or_else(|| "*".to_owned());
                        let optional = t.get("optional").map(toml_bool).unwrap_or(false);
                        (spec, optional)
                    }
                    _ => ("*".to_owned(), false),
                };
                deps.insert(
                    name.to_lowercase(),
                    DepInfo {
                        spec,
                        category: cat.to_owned(),
                        optional,
                        platform: false,
                        raw: name.clone(),
                        peer: false,
                        sha256: String::new(),
                    },
                );
            }
        }
    }
    if let Some(target) = data.get("target").and_then(|v| v.as_table()) {
        for tval in target.values() {
            let Some(tmap) = tval.as_table() else {
                continue;
            };
            for sub in ["dependencies", "dev-dependencies", "build-dependencies"] {
                let Some(tbl) = tmap.get(sub).and_then(|v| v.as_table()) else {
                    continue;
                };
                let cat = if sub == "dependencies" {
                    "prod"
                } else if sub.starts_with("dev") {
                    "dev"
                } else {
                    "build"
                };
                for (name, val) in tbl {
                    let spec = match val {
                        toml::Value::String(s) => s.clone(),
                        toml::Value::Table(t) => t
                            .get("version")
                            .and_then(toml_string)
                            .unwrap_or_else(|| "*".to_owned()),
                        _ => "*".to_owned(),
                    };
                    deps.insert(
                        name.to_lowercase(),
                        DepInfo {
                            spec,
                            category: cat.to_owned(),
                            optional: false,
                            platform: true,
                            raw: name.clone(),
                            peer: false,
                            sha256: String::new(),
                        },
                    );
                }
            }
        }
    }
    Ok(deps)
}

pub fn parse_rust_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::LockToml)?;
    let mut pkgs = BTreeMap::new();
    if let Some(list) = data.get("package").and_then(|v| v.as_array()) {
        for item in list {
            let name = item
                .get("name")
                .and_then(toml_string)
                .unwrap_or_default()
                .to_lowercase();
            let ver = item
                .get("version")
                .and_then(toml_string)
                .unwrap_or_default();
            if !name.is_empty() {
                pkgs.insert(name, ver);
            }
        }
    }
    Ok(pkgs)
}
