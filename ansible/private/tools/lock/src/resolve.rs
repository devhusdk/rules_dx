//! Resolves the collections the ansible manifest pins to their Galaxy archives.

use dx_lock::{Archive, Error, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The Galaxy endpoint the collection archives are downloaded from.
pub const SOURCE: &str =
    "https://galaxy.ansible.com/api/v3/plugin/ansible/content/published/collections/artifacts";

/// The resolver of the ansible ecosystem.
pub struct Ansible;

fn valid_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn valid_name(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(namespace, collection)| valid_part(namespace) && valid_part(collection))
}

fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

fn scalar(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(text)
}

/// The `name` and `version` of each entry under `collections:`, comments skipped.
///
/// # Errors
/// Fails when an entry has a key other than `name` or `version`, lacks an exact version, names a
/// collection that is not `namespace.collection`, a collection is listed twice, or no collection is
/// listed.
pub fn parse_requirements(text: &str) -> Result<Vec<(String, String)>, Error> {
    let mut entries: Vec<(Option<String>, Option<String>)> = Vec::new();
    let mut in_collections = false;
    for raw in text.lines() {
        let line = raw.split(" #").next().unwrap_or_default().trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "---" {
            continue;
        }
        if !line.starts_with([' ', '-']) {
            in_collections = trimmed == "collections:";
            if !in_collections && !trimmed.ends_with(':') {
                return Err(Error::new(format!("{trimmed:?} is not a top-level key")));
            }
            continue;
        }
        if !in_collections {
            continue;
        }
        let field = match trimmed.strip_prefix("- ") {
            Some(rest) => {
                entries.push((None, None));
                rest.trim_start()
            }
            None => trimmed,
        };
        let Some(entry) = entries.last_mut() else {
            return Err(Error::new(format!("{trimmed:?} is outside a collection entry")));
        };
        match field.split_once(':').map(|(key, value)| (key.trim(), scalar(value))) {
            Some(("name", value)) if entry.0.is_none() => entry.0 = Some(value.to_owned()),
            Some(("version", value)) if entry.1.is_none() => entry.1 = Some(value.to_owned()),
            _ => {
                return Err(Error::new(format!(
                    "{field:?} is not a name or version key of a collection entry"
                )));
            }
        }
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for entry in entries {
        let (Some(name), Some(version)) = entry else {
            return Err(Error::new("a collection entry needs a name and a version"));
        };
        if !valid_name(&name) {
            return Err(Error::new(format!(
                "{name:?} is not namespace.collection in lowercase letters, digits and underscores"
            )));
        }
        if !valid_version(&version) {
            return Err(Error::new(format!("{name} has no exact version: {version:?}")));
        }
        if out.iter().any(|(seen, _)| *seen == name) {
            return Err(Error::new(format!("{name} is listed twice")));
        }
        out.push((name, version));
    }
    if out.is_empty() {
        return Err(Error::new("no collection is listed"));
    }
    Ok(out)
}

impl Ansible {
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

impl Resolver for Ansible {
    fn resolve(&self, scratch: &Scratch, _old: Option<&Lock>) -> Result<Resolution, Error> {
        let manifest = dx_families::hub("ansible")
            .map(|hub| hub.manifest.clone())
            .ok_or_else(|| Error::new("ansible has no registry entry"))?;
        let text = std::fs::read_to_string(scratch.set().join(&manifest))
            .map_err(|error| Error::new(format!("{manifest}: {error}")))?;
        let entries =
            parse_requirements(&text).map_err(|error| Error::new(format!("{manifest}: {error}")))?;
        let packages = entries
            .into_iter()
            .map(|(name, version)| Package {
                archives: vec![Archive {
                    urls: vec![format!(
                        "{SOURCE}/{}-{version}.tar.gz",
                        name.replace('.', "-")
                    )],
                    kind: "tar.gz".to_owned(),
                    ..Archive::default()
                }],
                provides: vec![name.clone()],
                name,
                version,
                ..Package::default()
            })
            .collect();
        Ok(Resolution { packages })
    }
}
