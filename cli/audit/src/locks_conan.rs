use crate::vuln::LockedPackage;

pub const CONAN_SET: &str = "conan";

/// Reads the `name/version#revision` references of a `conan.lock`.
pub fn parse_conan_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("invalid conan.lock: {error}"))?;
    let Some(root) = root.as_object() else {
        return Err("invalid conan.lock: not an object".to_owned());
    };
    let mut out = Vec::new();
    let mut found = false;
    for key in ["requires", "build_requires", "python_requires"] {
        let Some(entries) = root.get(key) else {
            continue;
        };
        let entries = entries
            .as_array()
            .ok_or_else(|| format!("invalid conan.lock: {key} is not a list"))?;
        found = true;
        for entry in entries {
            let reference = entry
                .as_str()
                .ok_or_else(|| format!("invalid conan.lock: {key} entry is not a string"))?;
            let (name, version) = split_reference(reference)
                .ok_or_else(|| format!("invalid conan.lock: bad reference {reference:?}"))?;
            out.push(LockedPackage::new(CONAN_SET, name, version));
        }
    }
    if !found {
        return Err("invalid conan.lock: missing requires".to_owned());
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn split_reference(reference: &str) -> Option<(&str, &str)> {
    let without_revision = reference.split('#').next()?;
    let without_user = without_revision.split('@').next()?;
    let (name, version) = without_user.split_once('/')?;
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name, version))
}
