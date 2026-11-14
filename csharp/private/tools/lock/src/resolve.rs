//! The nuget resolver: one exact-version closure per Paket group of the Paket lock file.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::io::Cursor;

use base64::Engine as _;
use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use serde_json::Value;
use sha2::{Digest as _, Sha512};

/// The NuGet v3 service index that every Paket `source` names.
pub const SOURCE: &str = "https://api.nuget.org/v3/index.json";

/// The flat container of [`SOURCE`].
pub const FLAT: &str = "https://api.nuget.org/v3-flatcontainer";

/// The group a Paket file names before its first `group` line.
pub const MAIN: &str = "main";

/// Reads registry bytes.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// The NuGet registry over HTTPS.
pub struct Registry(HttpFetch);

impl Registry {
    /// A registry client.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Registry {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// One package line of a Paket lock group.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Pinned {
    /// The package id as Paket writes it.
    pub name: String,
    /// The normalized NuGet version.
    pub version: String,
    /// The ids of the packages it needs.
    pub deps: Vec<String>,
}

/// The group names of a Paket dependencies file, in file order.
///
/// # Errors
/// Fails on a source other than [`SOURCE`] and on a group named twice.
pub fn manifest_groups(text: &str) -> Result<Vec<String>, Error> {
    let mut groups = vec![MAIN.to_owned()];
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match (words.next(), words.next()) {
            (Some("group"), Some(name)) => {
                if groups.iter().any(|group| group.eq_ignore_ascii_case(name)) {
                    return Err(Error::new(format!("group {name} is named twice")));
                }
                groups.push(name.to_owned());
            }
            (Some("source"), Some(url)) if url != SOURCE => {
                return Err(Error::new(format!("source {url} is not {SOURCE}")));
            }
            (Some(kind @ ("github" | "gist" | "http" | "git" | "clitool")), _) => {
                return Err(Error::new(format!(
                    "a `{kind}` dependency has no NuGet archive"
                )));
            }
            _ => {}
        }
    }
    Ok(groups)
}

/// The NuGet normal form of a version: three or four numeric parts, a fourth only when not zero.
#[must_use]
pub fn normalize_version(version: &str) -> String {
    let (release, label) = match version.find(['-', '+']) {
        Some(at) => version.split_at(at),
        None => (version, ""),
    };
    let mut parts: Vec<String> = release
        .split('.')
        .map(|part| part.trim_start_matches('0'))
        .map(|part| if part.is_empty() { "0" } else { part }.to_owned())
        .collect();
    while parts.len() < 3 {
        parts.push("0".to_owned());
    }
    if parts.len() == 4 && parts.last().is_some_and(|part| part == "0") {
        parts.pop();
    }
    let label = label.split('+').next().unwrap_or_default();
    format!("{}{label}", parts.join("."))
}

/// The packages of each group of a Paket lock file, by group name.
///
/// # Errors
/// Fails on a section other than `NUGET`, a remote other than [`SOURCE`] and a line outside a package.
pub fn lock_groups(text: &str) -> Result<BTreeMap<String, Vec<Pinned>>, Error> {
    let mut groups: BTreeMap<String, Vec<Pinned>> = BTreeMap::new();
    let mut group = MAIN.to_owned();
    groups.entry(group.clone()).or_default();
    let mut in_nuget = false;
    for (number, line) in text.lines().enumerate() {
        let at = || format!("line {}", number + 1);
        let indent = line.len() - line.trim_start().len();
        let body = line.trim();
        if body.is_empty() {
            continue;
        }
        if indent == 0 {
            if let Some(name) = body.strip_prefix("GROUP ") {
                group = name.trim().to_owned();
                groups.entry(group.clone()).or_default();
                in_nuget = false;
            } else if body == "NUGET" {
                in_nuget = true;
            } else if matches!(body, "GITHUB" | "HTTP" | "GIT" | "GIST") {
                return Err(Error::new(format!(
                    "{}: a {body} section has no NuGet archive",
                    at()
                )));
            } else {
                in_nuget = false;
            }
            continue;
        }
        if !in_nuget {
            continue;
        }
        if let Some(remote) = body.strip_prefix("remote:") {
            if remote.trim() != SOURCE {
                return Err(Error::new(format!(
                    "{}: remote {} is not {SOURCE}",
                    at(),
                    remote.trim()
                )));
            }
            continue;
        }
        let (name, version) = package_line(body)
            .ok_or_else(|| Error::new(format!("{}: {body:?} is no package", at())))?;
        let packages = groups.entry(group.clone()).or_default();
        match indent {
            4 => packages.push(Pinned {
                name: name.to_owned(),
                version: normalize_version(version),
                deps: Vec::new(),
            }),
            6 => packages
                .last_mut()
                .ok_or_else(|| Error::new(format!("{}: a dependency before its package", at())))?
                .deps
                .push(name.to_owned()),
            _ => return Err(Error::new(format!("{}: unexpected indent {indent}", at()))),
        }
    }
    Ok(groups)
}

/// The id and parenthesized version of `Name (version) - options`.
fn package_line(body: &str) -> Option<(&str, &str)> {
    let (name, rest) = body.split_once(" (")?;
    let (version, _) = rest.split_once(')')?;
    Some((name.trim(), version.trim()))
}

/// The flat container URL of a package archive.
#[must_use]
pub fn archive_url(name: &str, version: &str) -> String {
    let id = name.to_ascii_lowercase();
    let version = version.to_ascii_lowercase();
    format!("{FLAT}/{id}/{version}/{id}.{version}.nupkg")
}

/// The assembly names under `lib/` of a package archive.
///
/// # Errors
/// Fails when the bytes are not a zip archive.
pub fn assemblies(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| Error::new(error.to_string()))?;
    let mut names = Vec::new();
    for path in archive.file_names() {
        let path = path.map_err(|error| Error::new(error.to_string()))?;
        let Some(file) = path
            .strip_prefix("lib/")
            .and_then(|rest| rest.rsplit('/').next())
        else {
            continue;
        };
        if let Some(stem) = file.len().checked_sub(4).and_then(|at| {
            file.get(at..)
                .filter(|suffix| suffix.eq_ignore_ascii_case(".dll"))
                .and(file.get(..at))
        }) {
            names.push(stem.to_owned());
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// The `sha512-<base64>` digest of `bytes`.
#[must_use]
pub fn sha512(bytes: &[u8]) -> String {
    format!(
        "sha512-{}",
        base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes))
    )
}

/// The nuget resolver.
pub struct Nuget<'a> {
    manifest: String,
    state: String,
    download: &'a dyn Download,
}

impl<'a> Nuget<'a> {
    /// A resolver for the files of `spec` over `download`.
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            state: spec.state.first().cloned().unwrap_or_default(),
            download,
        }
    }

    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }

    fn package(
        &self,
        pinned: &Pinned,
        names: &[&str],
        old: Option<&[Package]>,
    ) -> Result<Package, Error> {
        let url = archive_url(&pinned.name, &pinned.version);
        let deps: Vec<String> = pinned
            .deps
            .iter()
            .filter_map(|dep| names.iter().find(|name| name.eq_ignore_ascii_case(dep)))
            .map(|name| (*name).to_owned())
            .collect();
        let previous = old.and_then(|packages| {
            packages.iter().find(|package| {
                package.name == pinned.name
                    && package.version == pinned.version
                    && package
                        .archives
                        .iter()
                        .any(|archive| archive.urls == [url.clone()])
                    && package.extra.contains_key("nuget_sha512")
            })
        });
        if let Some(previous) = previous {
            return Ok(Package {
                deps,
                ..previous.clone()
            });
        }
        let bytes = self.download.bytes(&url).map_err(|error| {
            Error::new(format!(
                "package {} {}: {error}",
                pinned.name, pinned.version
            ))
        })?;
        let provides = assemblies(&bytes).map_err(|error| Error::new(format!("{url}: {error}")))?;
        Ok(Package {
            name: pinned.name.clone(),
            version: pinned.version.clone(),
            archives: vec![Archive {
                urls: vec![url],
                sha256: dx_digest::sha256_hex(&bytes),
                kind: "zip".to_owned(),
                ..Archive::default()
            }],
            deps,
            provides,
            extra: BTreeMap::from([("nuget_sha512".to_owned(), Value::String(sha512(&bytes)))]),
            ..Package::default()
        })
    }
}

impl Resolver for Nuget<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let read = |name: &str| {
            std::fs::read_to_string(scratch.set().join(name))
                .map_err(|error| Error::new(format!("{name}: {error}")))
        };
        let wanted = manifest_groups(&read(&self.manifest)?)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let locked = lock_groups(&read(&self.state)?)
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        let mut groups = BTreeMap::new();
        for group in wanted {
            let pinned = locked
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&group))
                .map(|(_, pinned)| pinned)
                .ok_or_else(|| Error::new(format!("{} has no group {group}", self.state)))?;
            if pinned.is_empty() {
                continue;
            }
            let names: Vec<&str> = pinned.iter().map(|package| package.name.as_str()).collect();
            let previous = old
                .and_then(|lock| lock.groups.get(&group))
                .map(Vec::as_slice);
            let packages = pinned
                .iter()
                .map(|package| self.package(package, &names, previous))
                .collect::<Result<Vec<_>, _>>()?;
            groups.insert(group, packages);
        }
        Ok(Resolution {
            packages: Vec::new(),
            groups,
        })
    }
}
