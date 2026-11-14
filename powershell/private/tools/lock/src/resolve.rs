//! Resolves the modules the PowerShell manifest pins to their Gallery packages.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_lock::{Archive, Error, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The Gallery API the packages are downloaded from.
pub const SOURCE: &str = "https://www.powershellgallery.com/api/v2";

/// The resolver of the powershell ecosystem.
pub struct Powershell;

/// The `Name = 'version'` entries of a requirements file, comments skipped.
///
/// # Errors
/// Fails when a line is not an entry, a module is listed twice, or the file lists no module.
pub fn parse_requirements(text: &str) -> Result<Vec<(String, String)>, Error> {
    let mut entries: Vec<(String, String)> = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.is_empty() || line == "@{" || line == "}" {
            continue;
        }
        let entry = line.split_once('=').and_then(|(name, version)| {
            let version = version.trim();
            let quote = version.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
            let version = version.strip_prefix(quote)?.strip_suffix(quote)?;
            Some((name.trim().trim_matches(['\'', '"']), version))
        });
        let Some((name, version)) = entry else {
            return Err(Error::new(format!("{line:?} is not a Name = 'version' entry")));
        };
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
        if !valid || version.is_empty() {
            return Err(Error::new(format!("{line:?} is not a Name = 'version' entry")));
        }
        if entries.iter().any(|(seen, _)| seen == name) {
            return Err(Error::new(format!("{name} is listed twice")));
        }
        entries.push((name.to_owned(), version.to_owned()));
    }
    if entries.is_empty() {
        return Err(Error::new("no module is listed"));
    }
    Ok(entries)
}

impl Powershell {
    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }
}

impl Resolver for Powershell {
    fn resolve(&self, scratch: &Scratch, _old: Option<&Lock>) -> Result<Resolution, Error> {
        let manifest = dx_families::hub("powershell")
            .map(|hub| hub.manifest.clone())
            .ok_or_else(|| Error::new("powershell has no registry entry"))?;
        let text = std::fs::read_to_string(scratch.set().join(&manifest))
            .map_err(|error| Error::new(format!("{manifest}: {error}")))?;
        let entries =
            parse_requirements(&text).map_err(|error| Error::new(format!("{manifest}: {error}")))?;
        let packages = entries
            .into_iter()
            .map(|(name, version)| Package {
                archives: vec![Archive {
                    urls: vec![format!("{SOURCE}/package/{name}/{version}")],
                    kind: "zip".to_owned(),
                    ..Archive::default()
                }],
                provides: vec![name.clone()],
                name,
                version,
                ..Package::default()
            })
            .collect();
        Ok(Resolution {
            groups: std::collections::BTreeMap::new(),
            packages,
            ..Resolution::default()
        })
    }
}
