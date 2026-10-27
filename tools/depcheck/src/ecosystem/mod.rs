pub mod cc;
pub mod dotnet;
pub mod go;
pub mod js;
pub mod jvm;
pub mod python;
pub mod ruby;
pub mod rust;

use std::collections::BTreeMap;

use crate::DepInfo;

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
