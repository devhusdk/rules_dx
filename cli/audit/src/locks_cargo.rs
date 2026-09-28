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
                name,
                version,
                set: "cargo".to_owned(),
                is_git: true,
                is_private: false,
            });
            continue;
        }
        if source.is_path() {
            continue;
        }
        out.push(LockedPackage {
            name,
            version,
            set: "cargo".to_owned(),
            is_git: false,
            is_private: false,
        });
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}
