use crate::vuln::LockedPackage;
use dx_error::DxError;

/// Reads `packages.<name>.version` of a pubspec.lock, keeping hosted packages only.
pub fn parse_pubspec_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let value: yaml_serde::Value = yaml_serde::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid pubspec.lock: {error}")))?;
    let packages = value
        .get("packages")
        .and_then(yaml_serde::Value::as_mapping)
        .ok_or_else(|| DxError::failure("invalid pubspec.lock: missing packages"))?;
    let mut out = Vec::new();
    for (name, detail) in packages {
        let name = name.as_str().ok_or_else(|| {
            DxError::failure("invalid pubspec.lock: package name is not a string")
        })?;
        let field = |key: &str| detail.get(key).and_then(yaml_serde::Value::as_str);
        if field("source") != Some("hosted") {
            continue;
        }
        let version = field("version").ok_or_else(|| {
            DxError::failure(format!("invalid pubspec.lock: {name} has no version"))
        })?;
        out.push(LockedPackage::new("pub", name, version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
