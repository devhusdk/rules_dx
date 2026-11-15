//! Locks the distributions a Carton snapshot pins for the modules the manifest requires.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};

use dx_lock::{Archive, Error, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The CPAN mirror a snapshot `pathname` is relative to.
pub const SOURCE: &str = "https://cpan.metacpan.org/authors/id/";

/// The resolver of the cpan ecosystem.
pub struct Cpan;

/// One `DISTRIBUTIONS` entry of a Carton snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Distribution {
    /// The entry key, `<name>-<version>`, which is also the archive's top directory.
    pub key: String,
    /// The archive path below [`SOURCE`].
    pub pathname: String,
    /// The Perl packages the distribution provides.
    pub provides: Vec<String>,
    /// The Perl modules the distribution requires.
    pub requirements: Vec<String>,
}

/// The distributions of a Carton snapshot, in file order.
///
/// # Errors
/// Fails when the text is not a version 1.0 snapshot, an entry has no `pathname`, or a key has no version.
pub fn parse_snapshot(text: &str) -> Result<Vec<Distribution>, Error> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("# carton snapshot format: version 1.0") {
        return Err(Error::new("is not a carton snapshot of format version 1.0"));
    }
    let mut out: Vec<Distribution> = Vec::new();
    let mut section = "";
    for line in lines {
        let indent = line.len() - line.trim_start().len();
        let body = line.trim();
        if body.is_empty() {
            continue;
        }
        match indent {
            0 => {
                if body != "DISTRIBUTIONS" {
                    return Err(Error::new(format!("{body:?} is not DISTRIBUTIONS")));
                }
            }
            2 => out.push(Distribution {
                key: body.to_owned(),
                ..Distribution::default()
            }),
            4 => {
                let Some(current) = out.last_mut() else {
                    return Err(Error::new(format!("{body:?} is outside a distribution")));
                };
                let (key, value) = body.split_once(':').unwrap_or((body, ""));
                section = key;
                if key == "pathname" {
                    current.pathname = value.trim().to_owned();
                }
            }
            _ => {
                let Some(current) = out.last_mut() else {
                    return Err(Error::new(format!("{body:?} is outside a distribution")));
                };
                let module = body.split_whitespace().next().unwrap_or_default().to_owned();
                match section {
                    "provides" => current.provides.push(module),
                    "requirements" => current.requirements.push(module),
                    _ => {}
                }
            }
        }
    }
    for distribution in &out {
        if distribution.pathname.is_empty() {
            return Err(Error::new(format!("{} has no pathname", distribution.key)));
        }
        if distribution.key.rsplit_once('-').is_none() {
            return Err(Error::new(format!("{} has no version", distribution.key)));
        }
    }
    if out.is_empty() {
        return Err(Error::new("pins no distribution"));
    }
    Ok(out)
}

/// The modules of the `requires` lines of the manifest.
#[must_use]
pub fn parse_requires(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("requires")?.trim_start();
            let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
            let (module, _) = rest.strip_prefix(quote)?.split_once(quote)?;
            Some(module.to_owned())
        })
        .collect()
}

/// The lock packages of a snapshot: each requirement becomes the distribution that provides it.
///
/// # Errors
/// Fails when the snapshot does, two distributions share a name, or a required module is in no distribution.
pub fn packages(manifest: &str, snapshot: &str) -> Result<Vec<Package>, Error> {
    let distributions = parse_snapshot(snapshot)?;
    let mut by_module: BTreeMap<&str, &str> = BTreeMap::new();
    let mut names = BTreeSet::new();
    for distribution in &distributions {
        let name = distribution
            .key
            .rsplit_once('-')
            .map_or("", |(name, _)| name);
        if !names.insert(name) {
            return Err(Error::new(format!("{name} is pinned twice")));
        }
        for module in &distribution.provides {
            by_module.entry(module).or_insert(name);
        }
    }
    for module in parse_requires(manifest) {
        if !by_module.contains_key(module.as_str()) && module != "perl" {
            return Err(Error::new(format!(
                "{module} is in no distribution of the snapshot; run carton install"
            )));
        }
    }
    Ok(distributions
        .iter()
        .map(|distribution| {
            let (name, version) = distribution
                .key
                .rsplit_once('-')
                .unwrap_or((distribution.key.as_str(), ""));
            let deps: BTreeSet<String> = distribution
                .requirements
                .iter()
                .filter_map(|module| by_module.get(module.as_str()))
                .filter(|dep| **dep != name)
                .map(|dep| (*dep).to_owned())
                .collect();
            Package {
                name: name.to_owned(),
                version: version.to_owned(),
                archives: vec![Archive {
                    urls: vec![format!("{SOURCE}{}", distribution.pathname)],
                    strip_prefix: distribution.key.clone(),
                    ..Archive::default()
                }],
                deps: deps.into_iter().collect(),
                provides: distribution.provides.clone(),
                ..Package::default()
            }
        })
        .collect())
}

impl Cpan {
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

impl Resolver for Cpan {
    fn resolve(&self, scratch: &Scratch, _old: Option<&Lock>) -> Result<Resolution, Error> {
        let hub = dx_families::hub("cpan").ok_or_else(|| Error::new("cpan has no registry entry"))?;
        let read = |name: &str| {
            std::fs::read_to_string(scratch.set().join(name))
                .map_err(|error| Error::new(format!("{name}: {error}")))
        };
        let state = hub
            .state
            .as_deref()
            .ok_or_else(|| Error::new("cpan has no state file"))?;
        let packages = packages(&read(&hub.manifest)?, &read(state)?)
            .map_err(|error| Error::new(format!("{state}: {error}")))?;
        Ok(Resolution {
            groups: std::collections::BTreeMap::new(),
            packages,
            ..Resolution::default()
        })
    }
}
