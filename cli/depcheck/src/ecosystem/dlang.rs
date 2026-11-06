use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::{normalize_lower, put};

/// Reads the `dependencies` of a `dub.json` or `dub.sdl`, chosen by the file extension.
/// A sub-package dependency (`vibe-d:http`) counts as its base package.
pub fn parse_d_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let mut deps = BTreeMap::new();
    if path.extension().is_some_and(|ext| ext == "sdl") {
        let line_re = regex::Regex::new(r#"^dependency\s+"([^"]+)"(.*)$"#)
            .map_err(DepcheckError::ManifestRegex)?;
        let version_re = regex::Regex::new(r#"\bversion\s*=\s*"([^"]*)""#)
            .map_err(DepcheckError::ManifestRegex)?;
        let optional_re =
            regex::Regex::new(r"\boptional\s*=\s*true\b").map_err(DepcheckError::ManifestRegex)?;
        for line in text.lines() {
            let Some(caps) = line_re.captures(line.trim()) else {
                continue;
            };
            let version = version_re.captures(&caps[2]).map(|v| v[1].to_owned());
            add_dep(
                &mut deps,
                &caps[1],
                version.as_deref(),
                optional_re.is_match(&caps[2]),
            );
        }
        return Ok(deps);
    }
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(DepcheckError::ManifestJson)?;
    for (name, value) in data
        .get("dependencies")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
    {
        let version = value
            .as_str()
            .or_else(|| value.get("version").and_then(|v| v.as_str()));
        let optional = value.get("optional").and_then(|v| v.as_bool()) == Some(true);
        add_dep(&mut deps, name, version, optional);
    }
    Ok(deps)
}

fn add_dep(
    deps: &mut BTreeMap<String, DepInfo>,
    name: &str,
    version: Option<&str>,
    optional: bool,
) {
    let base = name.split(':').next().unwrap_or(name);
    put(
        deps,
        normalize_lower(base),
        DepInfo {
            spec: d_spec(version.unwrap_or("")),
            category: "prod".to_owned(),
            optional,
            platform: false,
            raw: base.to_owned(),
            peer: false,
            sha256: String::new(),
            package: String::new(),
        },
    );
}

/// Dub separates range bounds with a space; the shared grammar uses commas.
fn d_spec(version: &str) -> String {
    let version = version.trim();
    if version.is_empty() || version.starts_with('~') && !version.starts_with("~>") {
        return "*".to_owned();
    }
    let mut spec = String::new();
    for part in version.split_whitespace() {
        if !spec.is_empty() {
            spec.push(',');
        }
        spec.push_str(part);
    }
    spec
}

/// Reads a `dub.selections.json`: package name to selected version. A path selection has none.
pub fn parse_d_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    let mut pkgs = BTreeMap::new();
    for (name, value) in data
        .get("versions")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
    {
        let version = value.as_str().unwrap_or("*");
        pkgs.insert(normalize_lower(name), version.to_owned());
    }
    Ok(pkgs)
}
