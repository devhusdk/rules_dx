//! Resolves an fpm package set from the git dependencies of its manifest and of every archive they name.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use std::collections::{BTreeMap, VecDeque};
use std::io::Read;

/// Downloads archives.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Github(HttpFetch);

impl Github {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Github {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Github {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// One git dependency of an fpm manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    /// The dependency name.
    pub name: String,
    /// The tag or commit.
    pub reference: String,
    /// The source archive of `reference`.
    pub url: String,
}

/// The source archive URL of `reference` in the GitHub repository `git`.
///
/// # Errors
/// Fails when `git` is not an `https://github.com/<owner>/<repo>` URL.
pub fn archive_url(git: &str, reference: &str) -> Result<String, Error> {
    let repo = git
        .strip_prefix("https://github.com/")
        .map(|rest| rest.trim_end_matches('/').trim_end_matches(".git"))
        .filter(|rest| {
            let parts: Vec<&str> = rest.split('/').collect();
            parts.len() == 2 && parts.iter().all(|part| !part.is_empty())
        })
        .ok_or_else(|| {
            Error::new(format!(
                "{git} is not supported; only https://github.com/<owner>/<repo> git URLs are"
            ))
        })?;
    Ok(format!(
        "https://github.com/{repo}/archive/{reference}.tar.gz"
    ))
}

/// The git dependencies of an fpm manifest; path dependencies are skipped.
///
/// # Errors
/// Fails when the text is not TOML or a dependency is not a GitHub git URL pinned to a tag or rev.
pub fn parse_dependencies(text: &str, with_dev: bool) -> Result<Vec<Dependency>, Error> {
    let data: toml::Table = toml::from_str(text).map_err(|error| Error::new(error.to_string()))?;
    let tables: &[&str] = if with_dev {
        &["dependencies", "dev-dependencies"]
    } else {
        &["dependencies"]
    };
    let mut out: Vec<Dependency> = Vec::new();
    for table in tables {
        let Some(entries) = data.get(*table) else {
            continue;
        };
        let entries = entries
            .as_table()
            .ok_or_else(|| Error::new(format!("[{table}] is not a table")))?;
        for (name, value) in entries {
            let field = |key: &str| value.get(key).and_then(toml::Value::as_str);
            if field("path").is_some() {
                continue;
            }
            let git = field("git").ok_or_else(|| {
                Error::new(format!(
                    "{name} is not supported; only git dependencies are"
                ))
            })?;
            let reference = field("tag").or_else(|| field("rev")).ok_or_else(|| {
                Error::new(format!("{name} must pin its git source with tag or rev"))
            })?;
            if out.iter().any(|seen| &seen.name == name) {
                return Err(Error::new(format!("{name} is listed twice")));
            }
            out.push(Dependency {
                name: name.clone(),
                reference: reference.to_owned(),
                url: archive_url(git, reference)?,
            });
        }
    }
    Ok(out)
}

/// The most bytes read from one archive member.
const MEMBER_LIMIT: u64 = 8 << 20;

/// The top directory of a `.tar.gz` and the `manifest` file inside it.
///
/// # Errors
/// Fails when the archive is unreadable, has no single top directory, or has no `manifest`.
pub fn read_archive(bytes: &[u8], manifest_name: &str) -> Result<(String, String), Error> {
    let failed = |error: std::io::Error| Error::new(error.to_string());
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut top: Option<String> = None;
    let mut manifest = None;
    for entry in archive.entries().map_err(failed)? {
        let mut entry = entry.map_err(failed)?;
        let path = entry.path().map_err(failed)?.to_string_lossy().into_owned();
        if path == "pax_global_header" {
            continue;
        }
        let first = path.split('/').next().unwrap_or_default().to_owned();
        match &top {
            Some(seen) if *seen != first => {
                return Err(Error::new("the archive has more than one top directory"));
            }
            Some(_) => {}
            None => top = Some(first.clone()),
        }
        if path.trim_end_matches('/') == format!("{first}/{manifest_name}") {
            let mut text = String::new();
            entry
                .by_ref()
                .take(MEMBER_LIMIT)
                .read_to_string(&mut text)
                .map_err(failed)?;
            manifest = Some(text);
        }
    }
    let top = top.ok_or_else(|| Error::new("the archive is empty"))?;
    let manifest = manifest.ok_or_else(|| Error::new(format!("the archive has no {manifest_name}")))?;
    Ok((top, manifest))
}

/// The resolver of the fpm ecosystem.
pub struct Fpm<'a> {
    manifest: String,
    download: &'a dyn Download,
}

impl<'a> Fpm<'a> {
    /// A resolver that downloads with `download`.
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
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

    fn describe(&self, dependency: &Dependency) -> Result<(Package, Vec<Dependency>), Error> {
        let name = &dependency.name;
        let failed = |error: Error| Error::new(format!("dependency {name}: {error}"));
        let bytes = self.download.bytes(&dependency.url).map_err(failed)?;
        let (strip_prefix, manifest) = read_archive(&bytes, &self.manifest).map_err(failed)?;
        let deps = parse_dependencies(&manifest, false).map_err(failed)?;
        let package = Package {
            name: name.clone(),
            version: dependency.reference.clone(),
            archives: vec![Archive {
                urls: vec![dependency.url.clone()],
                sha256: dx_digest::sha256_hex(&bytes),
                strip_prefix,
                platform: String::new(),
                kind: String::new(),
            }],
            deps: deps.iter().map(|dep| dep.name.clone()).collect(),
            provides: vec![name.clone()],
            dev: false,
            extra: BTreeMap::new(),
        };
        Ok((package, deps))
    }
}

fn kept<'l>(old: Option<&'l Lock>, dependency: &Dependency) -> Option<&'l Package> {
    old?.packages.iter().find(|package| {
        package.name == dependency.name
            && package.version == dependency.reference
            && matches!(
                package.archives.as_slice(),
                [archive] if archive.urls == [dependency.url.clone()] && !archive.sha256.is_empty()
            )
    })
}

fn from_lock(old: Option<&Lock>, name: &str) -> Option<Dependency> {
    let package = old?.packages.iter().find(|package| package.name == name)?;
    Some(Dependency {
        name: package.name.clone(),
        reference: package.version.clone(),
        url: package.archives.first()?.urls.first()?.clone(),
    })
}

impl Resolver for Fpm<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let text = std::fs::read_to_string(scratch.set().join(&self.manifest))
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let roots = parse_dependencies(&text, true)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let mut chosen: BTreeMap<String, (Dependency, String)> = BTreeMap::new();
        let mut packages: BTreeMap<String, Package> = BTreeMap::new();
        let mut queue: VecDeque<(Dependency, String)> = roots
            .into_iter()
            .map(|dependency| (dependency, self.manifest.clone()))
            .collect();
        while let Some((dependency, by)) = queue.pop_front() {
            if let Some((seen, first)) = chosen.get(&dependency.name) {
                if seen.url != dependency.url {
                    return Err(Error::new(format!(
                        "{} is wanted at {} by {first} and at {} by {by}",
                        dependency.name, seen.reference, dependency.reference
                    )));
                }
                continue;
            }
            chosen.insert(dependency.name.clone(), (dependency.clone(), by));
            let (package, deps) = match kept(old, &dependency) {
                Some(package) => {
                    let deps: Option<Vec<Dependency>> =
                        package.deps.iter().map(|dep| from_lock(old, dep)).collect();
                    match deps {
                        Some(deps) => (package.clone(), deps),
                        None => self.describe(&dependency)?,
                    }
                }
                None => self.describe(&dependency)?,
            };
            for dep in deps {
                queue.push_back((dep, dependency.name.clone()));
            }
            packages.insert(package.name.clone(), package);
        }
        Ok(Resolution {
            packages: packages.into_values().collect(),
        })
    }
}
