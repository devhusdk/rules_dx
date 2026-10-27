use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::version::satisfies_for;
use crate::{DepInfo, DepcheckError, Ecosystem};

pub fn normalize_js(name: &str) -> String {
    name.to_lowercase()
}

const CONCRETE_FIELDS: [(&str, &str, bool); 3] = [
    ("dependencies", "prod", false),
    ("optionalDependencies", "prod", true),
    ("devDependencies", "dev", false),
];

fn manifest_section<'a>(
    data: &'a serde_json::Value,
    field: &str,
) -> Result<Option<&'a serde_json::Map<String, serde_json::Value>>, DepcheckError> {
    match data.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(DepcheckError::ManifestParse(format!(
            "{field} must be an object"
        ))),
    }
}

fn manifest_spec(
    field: &str,
    name: &str,
    spec: &serde_json::Value,
) -> Result<String, DepcheckError> {
    spec.as_str()
        .map(str::to_owned)
        .ok_or_else(|| DepcheckError::ManifestParse(format!("{field}.{name} must be a string")))
}

pub fn parse_js_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(DepcheckError::ManifestJson)?;
    let mut deps: BTreeMap<String, DepInfo> = BTreeMap::new();
    for (field, category, optional) in CONCRETE_FIELDS {
        let Some(map) = manifest_section(&data, field)? else {
            continue;
        };
        for (name, spec) in map {
            let spec = manifest_spec(field, name, spec)?;
            let key = normalize_js(name);
            if let Some(prev) = deps.get(&key) {
                if prev.spec != spec {
                    return Err(DepcheckError::ManifestParse(format!(
                        "{name} is declared with conflicting requirements: {} and {spec}",
                        prev.spec
                    )));
                }
                continue;
            }
            deps.insert(
                key,
                DepInfo {
                    spec,
                    category: category.to_owned(),
                    optional,
                    platform: optional,
                    raw: name.clone(),
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
    }
    if let Some(map) = manifest_section(&data, "peerDependencies")? {
        for (name, spec) in map {
            let spec = manifest_spec("peerDependencies", name, spec)?;
            deps.entry(normalize_js(name)).or_insert_with(|| DepInfo {
                spec,
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: name.clone(),
                peer: true,
                sha256: String::new(),
                package: String::new(),
            });
        }
    }
    Ok(deps)
}

/// One direct dependency as an importer records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnpmEntry {
    pub specifier: String,
    pub version: String,
}

/// A pnpm lock split into importers and the package identities it lists.
#[derive(Debug, Default)]
pub struct PnpmLock {
    importers: BTreeMap<String, BTreeMap<String, PnpmEntry>>,
    packages: BTreeSet<(String, String)>,
}

impl PnpmLock {
    pub fn importer(&self, key: &str) -> Option<&BTreeMap<String, PnpmEntry>> {
        self.importers.get(key)
    }

    pub fn importer_keys(&self) -> Vec<&str> {
        self.importers.keys().map(String::as_str).collect()
    }

    pub fn has_package(&self, name: &str, version: &str) -> bool {
        self.packages
            .contains(&(normalize_js(name), version.to_owned()))
    }
}

fn lock_error(message: impl Into<String>) -> DepcheckError {
    DepcheckError::LockParse(message.into())
}

fn normalize_importer_key(key: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in key.trim().split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

fn dir_of(path: &Path) -> std::path::PathBuf {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}

/// Names the lock importer that belongs to a manifest.
pub fn pnpm_importer_key(manifest: &Path, lock: &Path) -> Result<String, DepcheckError> {
    let manifest_dir =
        std::fs::canonicalize(dir_of(manifest)).map_err(DepcheckError::ManifestIo)?;
    let lock_dir = std::fs::canonicalize(dir_of(lock)).map_err(DepcheckError::LockIo)?;
    let rel = manifest_dir.strip_prefix(&lock_dir).map_err(|_| {
        lock_error(format!(
            "{} is outside the workspace of {}",
            manifest.display(),
            lock.display()
        ))
    })?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Ok(normalize_importer_key(&parts.join("/")))
}

fn string_field(
    map: &yaml_serde::Mapping,
    field: &str,
    context: &str,
) -> Result<String, DepcheckError> {
    match map.get(field) {
        Some(yaml_serde::Value::String(text)) if !text.trim().is_empty() => Ok(text.clone()),
        _ => Err(lock_error(format!("{context} needs a string {field}"))),
    }
}

fn parse_importer(
    key: &str,
    value: &yaml_serde::Value,
) -> Result<BTreeMap<String, PnpmEntry>, DepcheckError> {
    let mut entries: BTreeMap<String, PnpmEntry> = BTreeMap::new();
    let map = match value {
        yaml_serde::Value::Null => return Ok(entries),
        yaml_serde::Value::Mapping(map) => map,
        _ => return Err(lock_error(format!("importer '{key}' must be a mapping"))),
    };
    for section in ["dependencies", "optionalDependencies", "devDependencies"] {
        let deps = match map.get(section) {
            None | Some(yaml_serde::Value::Null) => continue,
            Some(yaml_serde::Value::Mapping(deps)) => deps,
            Some(_) => {
                return Err(lock_error(format!(
                    "importer '{key}' {section} must be a mapping"
                )))
            }
        };
        for (name, entry) in deps {
            let yaml_serde::Value::String(name) = name else {
                return Err(lock_error(format!(
                    "importer '{key}' {section} has a non-string name"
                )));
            };
            let context = format!("importer '{key}' {section}.{name}");
            let yaml_serde::Value::Mapping(entry) = entry else {
                return Err(lock_error(format!(
                    "{context} must be a mapping with specifier and version"
                )));
            };
            let parsed = PnpmEntry {
                specifier: string_field(entry, "specifier", &context)?,
                version: string_field(entry, "version", &context)?,
            };
            match entries.get(&normalize_js(name)) {
                Some(prev) if *prev != parsed => {
                    return Err(lock_error(format!(
                        "importer '{key}' resolves '{name}' more than once"
                    )))
                }
                Some(_) => {}
                None => {
                    entries.insert(normalize_js(name), parsed);
                }
            }
        }
    }
    Ok(entries)
}

fn package_identity(key: &str) -> Result<(String, String), DepcheckError> {
    let trimmed = key.trim().trim_start_matches('/');
    let bare = trimmed.split('(').next().unwrap_or("");
    match bare.rfind('@') {
        Some(at) if at > 0 && at + 1 < bare.len() => {
            Ok((normalize_js(&bare[..at]), bare[at + 1..].to_owned()))
        }
        _ => Err(lock_error(format!("malformed package key '{key}'"))),
    }
}

fn collect_packages(
    root: &yaml_serde::Mapping,
    out: &mut BTreeSet<(String, String)>,
) -> Result<(), DepcheckError> {
    for section in ["packages", "snapshots"] {
        match root.get(section) {
            None | Some(yaml_serde::Value::Null) => {}
            Some(yaml_serde::Value::Mapping(map)) => {
                for key in map.keys() {
                    let yaml_serde::Value::String(key) = key else {
                        return Err(lock_error(format!("{section} has a non-string key")));
                    };
                    out.insert(package_identity(key)?);
                }
            }
            Some(_) => return Err(lock_error(format!("{section} must be a mapping"))),
        }
    }
    Ok(())
}

pub fn parse_pnpm_lock(path: &Path) -> Result<PnpmLock, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let value: yaml_serde::Value =
        yaml_serde::from_str(&text).map_err(|err| lock_error(err.to_string()))?;
    let yaml_serde::Value::Mapping(root) = &value else {
        return Err(lock_error("pnpm lock must be a mapping"));
    };
    let mut lock = PnpmLock::default();
    match root.get("importers") {
        Some(yaml_serde::Value::Mapping(importers)) => {
            for (key, body) in importers {
                let yaml_serde::Value::String(key) = key else {
                    return Err(lock_error("importers has a non-string key"));
                };
                let norm = normalize_importer_key(key);
                if lock.importers.contains_key(&norm) {
                    return Err(lock_error(format!("importer '{norm}' is listed twice")));
                }
                lock.importers.insert(norm, parse_importer(key, body)?);
            }
        }
        None | Some(yaml_serde::Value::Null) => {
            let single = ["dependencies", "optionalDependencies", "devDependencies"]
                .iter()
                .any(|field| root.get(*field).is_some());
            if single {
                lock.importers
                    .insert(".".to_owned(), parse_importer(".", &value)?);
            }
        }
        Some(_) => return Err(lock_error("importers must be a mapping")),
    }
    collect_packages(root, &mut lock.packages)?;
    Ok(lock)
}

const LOCAL_PREFIXES: [&str; 3] = ["workspace:", "link:", "file:"];

fn is_local(text: &str) -> bool {
    LOCAL_PREFIXES.iter().any(|p| text.starts_with(p))
}

fn is_remote_spec(spec: &str) -> bool {
    const PREFIXES: [&str; 6] = ["github:", "gitlab:", "bitbucket:", "git+", "git:", "git@"];
    if spec.contains("://") || PREFIXES.iter().any(|p| spec.starts_with(p)) {
        return true;
    }
    let shorthand = spec.split('#').next().unwrap_or("");
    !spec.starts_with('@')
        && !spec.starts_with("npm:")
        && !spec.contains(':')
        && shorthand.matches('/').count() == 1
        && !shorthand.starts_with('/')
        && !shorthand.ends_with('/')
        && !shorthand.contains(char::is_whitespace)
}

fn split_alias(text: &str) -> Option<(&str, &str)> {
    let at = text.rfind('@')?;
    (at > 0 && at + 1 < text.len()).then(|| (&text[..at], &text[at + 1..]))
}

/// Compares each manifest declaration with the importer's resolution.
pub fn pnpm_failures(
    deps: &BTreeMap<String, DepInfo>,
    importer: &BTreeMap<String, PnpmEntry>,
    lock: &PnpmLock,
) -> Result<Vec<String>, DepcheckError> {
    let mut failures = Vec::new();
    for (name, info) in deps {
        let spec = info.spec.trim();
        if spec.starts_with("catalog:") {
            return Err(lock_error(format!(
                "'{}' uses the unsupported specifier {spec}",
                info.raw
            )));
        }
        let Some(entry) = importer.get(name) else {
            failures.push(format!(
                "missing lock entry for '{}' (manifest requires {})",
                info.raw, info.spec
            ));
            continue;
        };
        let stale = |locked: &str| {
            format!(
                "stale lock entry for '{}': manifest requires {}, lock has {locked}",
                info.raw, info.spec
            )
        };
        if entry.specifier.trim() != spec {
            failures.push(format!(
                "stale lock entry for '{}': manifest requires {}, lock importer specifier is {}",
                info.raw, info.spec, entry.specifier
            ));
            continue;
        }
        let version = entry.version.trim();
        if is_local(spec) || is_local(version) {
            if !(is_local(spec) && is_local(version)) {
                failures.push(stale(version));
            }
            continue;
        }
        if is_remote_spec(spec) || version.contains("://") {
            continue;
        }
        let resolved = version.split('(').next().unwrap_or("").trim();
        let (target, range, locked) = if let Some(alias) = spec.strip_prefix("npm:") {
            let (Some((want, range)), Some((got, locked))) =
                (split_alias(alias), split_alias(resolved))
            else {
                failures.push(stale(version));
                continue;
            };
            if normalize_js(want) != normalize_js(got) {
                failures.push(stale(version));
                continue;
            }
            (got, range, locked)
        } else {
            (info.raw.as_str(), spec, resolved)
        };
        if !satisfies_for(Ecosystem::Js, range, locked) {
            failures.push(stale(version));
        } else if !lock.has_package(target, locked) {
            failures.push(format!(
                "missing package entry for '{}': lock selects {target}@{locked} but lists no such package",
                info.raw
            ));
        }
    }
    Ok(failures)
}
