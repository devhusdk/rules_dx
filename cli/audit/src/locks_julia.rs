use crate::vuln::LockedPackage;
use dx_error::DxError;

const JULIA_SET: &str = "julia";

/// Reads `[[deps.Name]]` entries of a format 2.0 Manifest.toml; stdlibs without a version are skipped.
pub fn parse_julia_manifest(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let table: toml::Table = toml::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid Manifest.toml: {error}")))?;
    match table.get("manifest_format").and_then(toml::Value::as_str) {
        Some("2.0") => {}
        Some(other) => {
            return Err(DxError::failure(format!(
                "unsupported Manifest.toml format {other}; regenerate it with Julia 1.7 or newer (format 2.0)"
            )));
        }
        None => {
            return Err(DxError::failure(
                "unsupported Manifest.toml format 1.0; regenerate it with Julia 1.7 or newer (format 2.0)",
            ));
        }
    }
    let mut out = Vec::new();
    if let Some(deps) = table.get("deps").and_then(toml::Value::as_table) {
        for (name, entries) in deps {
            let entries = entries.as_array().ok_or_else(|| {
                DxError::failure(format!("invalid Manifest.toml: deps.{name} is not a list"))
            })?;
            for entry in entries {
                let Some(version) = entry.get("version").and_then(toml::Value::as_str) else {
                    continue;
                };
                out.push(LockedPackage::new(JULIA_SET, name, version));
            }
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}
