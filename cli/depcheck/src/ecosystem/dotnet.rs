use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::normalize_lower;

/// Reads the `nuget` lines of a Paket manifest, with their groups.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or does not parse.
pub fn parse_dotnet_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let mut deps = BTreeMap::new();
    let mut group = "Main".to_owned();
    let group_re =
        regex::Regex::new(r"(?i)^group\s+(\S+)").map_err(DepcheckError::ManifestRegex)?;
    let nuget_re = regex::Regex::new(r"(?i)^nuget\s+(\S+)\s+(\S+)(.*)$")
        .map_err(DepcheckError::ManifestRegex)?;
    let optional_re =
        regex::Regex::new(r"(?i)//\s*optional\b").map_err(DepcheckError::ManifestRegex)?;
    let platform_re =
        regex::Regex::new(r"(?i)//\s*platform\b").map_err(DepcheckError::ManifestRegex)?;
    for rawline in text.lines() {
        let line = rawline.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let low = line.to_lowercase();
        if low.starts_with("source ") || low.starts_with("framework:") {
            continue;
        }
        if let Some(caps) = group_re.captures(line) {
            caps[1].clone_into(&mut group);
            continue;
        }
        let Some(caps) = nuget_re.captures(line) else {
            continue;
        };
        let name = caps[1].to_owned();
        let ver = caps[2].to_owned();
        let rest = caps.get(3).map_or("", |m| m.as_str());
        let category = if ["main", "prod", "compile"].contains(&group.to_lowercase().as_str()) {
            "prod"
        } else {
            "dev"
        };
        deps.insert(
            normalize_lower(&name),
            DepInfo {
                spec: ver,
                category: category.to_owned(),
                optional: optional_re.is_match(rest),
                platform: platform_re.is_match(rest),
                raw: name,
                peer: false,
                sha256: String::new(),
                package: String::new(),
            },
        );
    }
    Ok(deps)
}
