use crate::vuln::LockedPackage;
use dx_error::DxError;

pub const LUAROCKS_SET: &str = "luarocks";

/// Reads the packages of a lock.
pub fn parse_luarocks_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let invalid = |detail: &str| DxError::failure(format!("invalid lock: {detail}"));
    let lock: serde_json::Value =
        serde_json::from_str(text).map_err(|error| invalid(&error.to_string()))?;
    let packages = lock
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid("missing packages"))?;
    let mut out = Vec::new();
    for package in packages {
        let field = |key: &str| {
            package
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
        };
        let (Some(name), Some(version)) = (field("name"), field("version")) else {
            return Err(invalid("a package has no name or version"));
        };
        out.push(LockedPackage::new(LUAROCKS_SET, name, version.trim()));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
