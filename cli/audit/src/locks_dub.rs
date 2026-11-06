use crate::vuln::LockedPackage;

pub const DUB_SET: &str = "dub";

/// Reads the `versions` of a `dub.selections.json`. A version is a string or `{"version": ...}`.
pub fn parse_dub_selections(text: &str) -> Result<Vec<LockedPackage>, String> {
    let root: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid dub.selections.json: {error}"))?;
    let versions = root
        .get("versions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "invalid dub.selections.json: missing versions".to_owned())?;
    let mut out = Vec::new();
    for (name, entry) in versions {
        let version = entry
            .as_str()
            .or_else(|| entry.get("version").and_then(serde_json::Value::as_str))
            .ok_or_else(|| format!("invalid dub.selections.json: no version for {name}"))?;
        out.push(LockedPackage::new(DUB_SET, name, version));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
