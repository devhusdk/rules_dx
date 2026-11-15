use crate::vuln::LockedPackage;
use dx_error::DxError;

/// Reads the packages of a hex lock.
pub fn parse_hex_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid hex lock: {error}")))?;
    let packages = value
        .get("packages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| DxError::failure("invalid hex lock: missing packages"))?;
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
                "invalid hex lock: package without a version",
            ));
        }
        out.push(LockedPackage::new("hex", name, version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
