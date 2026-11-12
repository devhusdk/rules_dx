use crate::vuln::LockedPackage;
use dx_error::DxError;

pub const FPM_SET: &str = "fpm";

/// Reads the packages of an fpm lock.
pub fn parse_fpm_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid fpm lock: {error}")))?;
    let packages = value
        .get("packages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| DxError::failure("invalid fpm lock: missing packages"))?;
    let mut out = Vec::new();
    for package in packages {
        let field = |key: &str| {
            package
                .get(key)
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
        };
        let (name, version) = (field("name"), field("version"));
        if name.is_empty() || version.is_empty() {
            return Err(DxError::failure(
                "invalid fpm lock: package without a version",
            ));
        }
        out.push(LockedPackage::new(FPM_SET, name, version));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
