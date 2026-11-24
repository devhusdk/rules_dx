pub mod ansible;
pub mod beam;
pub mod cc;
pub mod dart;
pub mod dlang;
pub mod dotnet;
pub mod fortran;
pub mod go;
pub mod haskell;
pub mod js;
pub mod julia;
pub mod jvm;
pub mod lua;
pub mod ocaml;
pub mod perl;
pub mod php;
pub mod powershell;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod zig;

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

pub(crate) fn lock_packages(path: &Path) -> Result<Vec<(String, Option<String>)>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    Ok(data
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?.to_owned();
            let version = package
                .get("version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            Some((name, version))
        })
        .collect())
}

#[must_use]
pub fn normalize_lower(name: &str) -> String {
    name.to_lowercase()
}

#[must_use]
pub fn normalize_snake(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

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
