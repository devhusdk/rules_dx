use crate::vuln::LockedPackage;

pub fn parse_cargo_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let lockfile: cargo_lock::Lockfile = text
        .parse()
        .map_err(|error: cargo_lock::Error| format!("invalid Cargo.lock: {error}"))?;
    if lockfile.packages.is_empty() {
        return Err("invalid Cargo.lock: missing [[package]]".to_owned());
    }
    let mut out = Vec::new();
    for package in &lockfile.packages {
        let name = package.name.as_str().to_owned();
        let version = package.version.to_string();
        if name.trim().is_empty() || version.trim().is_empty() {
            return Err("invalid Cargo.lock: empty package name or version".to_owned());
        }
        let Some(source) = &package.source else {
            continue;
        };
        if source.is_git() {
            out.push(LockedPackage {
                is_git: true,
                ..LockedPackage::new("cargo", name, version)
            });
            continue;
        }
        if source.is_path() {
            continue;
        }
        out.push(LockedPackage::new("cargo", name, version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

/// One `[[package]]` of a Cargo.lock with the keys that decide its build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoLockEntry {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
    pub dependencies: Vec<(String, String)>,
}

pub fn cargo_lock_graph(text: &str) -> Result<Vec<CargoLockEntry>, String> {
    let lockfile: cargo_lock::Lockfile = text
        .parse()
        .map_err(|error: cargo_lock::Error| format!("invalid Cargo.lock: {error}"))?;
    if lockfile.packages.is_empty() {
        return Err("invalid Cargo.lock: missing [[package]]".to_owned());
    }
    let mut out: Vec<CargoLockEntry> = lockfile
        .packages
        .iter()
        .map(|package| {
            let mut dependencies: Vec<(String, String)> = package
                .dependencies
                .iter()
                .map(|dependency| {
                    (
                        dependency.name.as_str().to_owned(),
                        dependency.version.to_string(),
                    )
                })
                .collect();
            dependencies.sort();
            CargoLockEntry {
                name: package.name.as_str().to_owned(),
                version: package.version.to_string(),
                source: package.source.as_ref().map(ToString::to_string),
                checksum: package.checksum.as_ref().map(ToString::to_string),
                dependencies,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
