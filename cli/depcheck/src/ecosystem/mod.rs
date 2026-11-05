pub mod cc;
pub mod dotnet;
pub mod go;
pub mod js;
pub mod jvm;
pub mod python;
pub mod ruby;
pub mod rust;

use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

fn rank(cat: &str) -> u8 {
    match cat {
        "prod" => 3,
        "build" => 2,
        _ => 1,
    }
}

pub(crate) fn put(deps: &mut BTreeMap<String, DepInfo>, key: String, info: DepInfo) {
    match deps.get(&key) {
        Some(old) if rank(&info.category) <= rank(&old.category) => {}
        _ => {
            deps.insert(key, info);
        }
    }
}

pub(crate) fn push_version(pkgs: &mut crate::LockVersions, name: String, ver: String) {
    let versions = pkgs.entry(name).or_default();
    if !versions.contains(&ver) {
        versions.push(ver);
    }
}

/// Lowercase-only package key (Go, JVM, .NET, JS/TS, Ruby, Rust).
pub fn normalize_lower(name: &str) -> String {
    name.to_lowercase()
}

/// Lowercase package key with `-` folded to `_` (C/C++).
pub fn normalize_snake(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

/// Parses a paren-style lock (`paket.lock`, `Gemfile.lock`): every 4-space
/// indented `name (version)` line, keyed by `normalize_lower`.
pub fn parse_paren_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let re = regex::Regex::new(r"^    ([A-Za-z0-9_.\-]+) \(([^)]+)\)")
        .map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    for line in text.lines() {
        if let Some(caps) = re.captures(line) {
            pkgs.insert(normalize_lower(&caps[1]), caps[2].trim().to_owned());
        }
    }
    Ok(pkgs)
}
