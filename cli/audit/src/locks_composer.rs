use crate::vuln::LockedPackage;
use dx_error::DxError;

pub const COMPOSER_SET: &str = "packagist";

/// Reads the `packages` and `packages-dev` of a composer.lock, skipping branch versions.
pub fn parse_composer_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let root: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid composer.lock: {error}")))?;
    let mut out = Vec::new();
    for key in ["packages", "packages-dev"] {
        let Some(entries) = root.get(key) else {
            continue;
        };
        let entries = entries.as_array().ok_or_else(|| {
            DxError::failure(format!("invalid composer.lock: {key} is not a list"))
        })?;
        for entry in entries {
            let field = |name: &str| entry.get(name).and_then(serde_json::Value::as_str);
            let name = field("name").ok_or_else(|| {
                DxError::failure(format!("invalid composer.lock: {key} entry has no name"))
            })?;
            let version = field("version").ok_or_else(|| {
                DxError::failure(format!("invalid composer.lock: {name} has no version"))
            })?;
            if version.starts_with("dev-") || version.ends_with("-dev") {
                continue;
            }
            let version = version.strip_prefix('v').unwrap_or(version);
            out.push(LockedPackage::new(COMPOSER_SET, name, version));
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
