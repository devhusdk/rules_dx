use crate::vuln::LockedPackage;

pub fn parse_psgallery_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid PSGallery.lock.json: {error}"))?;
    let modules = value
        .get("modules")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "invalid PSGallery.lock.json: missing modules".to_owned())?;
    let mut out = Vec::new();
    for (name, detail) in modules {
        let name = name.trim();
        let version = detail
            .get("version")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        if name.is_empty() || version.is_empty() {
            return Err("invalid PSGallery.lock.json: module without a version".to_owned());
        }
        out.push(LockedPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            set: "nuget".to_owned(),
            is_git: false,
            is_private: false,
        });
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
