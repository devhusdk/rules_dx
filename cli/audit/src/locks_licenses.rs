use super::package_lock_name;
use crate::vuln::LockedPackage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicensedPackage {
    pub name: String,
    pub version: String,
    pub set: String,
    pub license: String,
}

pub fn cargo_licenses(cargo_bazel_text: &str, packages: &[LockedPackage]) -> Vec<LicensedPackage> {
    let value: serde_json::Value = match serde_json::from_str(cargo_bazel_text) {
        Ok(value) => value,
        Err(_) => {
            return packages
                .iter()
                .filter(|package| package.set == "cargo")
                .map(|package| LicensedPackage {
                    name: package.name.clone(),
                    version: package.version.clone(),
                    set: package.set.clone(),
                    license: "UNKNOWN".to_owned(),
                })
                .collect();
        }
    };
    let mut by_name_version = std::collections::BTreeMap::new();
    for key in ["crates", "packages"] {
        if let Some(packages_map) = value.get(key).and_then(|value| value.as_object()) {
            for (key, detail) in packages_map {
                let license = detail
                    .get("license")
                    .and_then(|value| value.as_str())
                    .unwrap_or("UNKNOWN")
                    .trim();
                let license = if license.is_empty() {
                    "UNKNOWN"
                } else {
                    license
                };
                by_name_version.insert(key.clone(), license.to_owned());
            }
        }
    }
    packages
        .iter()
        .filter(|package| package.set == "cargo")
        .map(|package| {
            let key = format!("{} {}", package.name, package.version);
            let license = by_name_version
                .get(&key)
                .cloned()
                .unwrap_or_else(|| "UNKNOWN".to_owned());
            LicensedPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                set: package.set.clone(),
                license,
            }
        })
        .collect()
}

pub fn unknown_licenses(packages: &[LockedPackage], set: &str) -> Vec<LicensedPackage> {
    packages
        .iter()
        .filter(|package| package.set == set)
        .map(|package| LicensedPackage {
            name: package.name.clone(),
            version: package.version.clone(),
            set: package.set.clone(),
            license: "UNKNOWN".to_owned(),
        })
        .collect()
}

pub fn npm_licenses(package_lock_text: &str, packages: &[LockedPackage]) -> Vec<LicensedPackage> {
    let mut by_name_version = std::collections::BTreeMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(package_lock_text) {
        if let Some(object) = value.as_object() {
            if let Some(detail_map) = object.get("packages").and_then(|v| v.as_object()) {
                for (path, detail) in detail_map {
                    let Some(name) = package_lock_name(path) else {
                        continue;
                    };
                    let detail = match detail.as_object() {
                        Some(detail) => detail,
                        None => continue,
                    };
                    let version = detail
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if version.is_empty()
                        || version.starts_with("file:")
                        || version.starts_with("link:")
                    {
                        continue;
                    }
                    let license = detail
                        .get("license")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNKNOWN")
                        .trim();
                    let license = if license.is_empty() {
                        "UNKNOWN"
                    } else {
                        license
                    };
                    by_name_version.insert((name, version.to_owned()), license.to_owned());
                }
            }
            if let Some(detail_map) = object.get("dependencies").and_then(|v| v.as_object()) {
                for (name, detail) in detail_map {
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let detail = match detail.as_object() {
                        Some(detail) => detail,
                        None => continue,
                    };
                    let version = detail
                        .get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if version.is_empty() {
                        continue;
                    }
                    let license = detail
                        .get("license")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNKNOWN")
                        .trim();
                    let license = if license.is_empty() {
                        "UNKNOWN"
                    } else {
                        license
                    };
                    by_name_version
                        .entry((name.to_owned(), version.to_owned()))
                        .or_insert_with(|| license.to_owned());
                }
            }
        }
    }
    packages
        .iter()
        .filter(|package| package.set == "npm")
        .map(|package| {
            let license = by_name_version
                .get(&(package.name.clone(), package.version.clone()))
                .cloned()
                .unwrap_or_else(|| "UNKNOWN".to_owned());
            LicensedPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                set: package.set.clone(),
                license,
            }
        })
        .collect()
}

pub fn inventory_licenses(
    packages: &[LockedPackage],
    inventory: &[crate::license_policy::LicenseInventory],
    set: &str,
) -> Vec<LicensedPackage> {
    packages
        .iter()
        .filter(|package| package.set == set)
        .map(|package| {
            let mut license = "UNKNOWN".to_owned();
            for entry in inventory {
                if entry.set != set || entry.package != package.name {
                    continue;
                }
                if crate::vuln::version_affected(set, &entry.versions, &package.version) {
                    license = entry.license.clone();
                    break;
                }
            }
            LicensedPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                set: package.set.clone(),
                license,
            }
        })
        .collect()
}

pub fn inventory_text_present(
    package: &LockedPackage,
    inventory: &[crate::license_policy::LicenseInventory],
) -> bool {
    for entry in inventory {
        if entry.set != package.set || entry.package != package.name {
            continue;
        }
        if crate::vuln::version_affected(&package.set, &entry.versions, &package.version) {
            return entry.text_present;
        }
    }
    false
}
