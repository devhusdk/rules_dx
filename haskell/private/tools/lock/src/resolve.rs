#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! Resolves a cabal project into its freeze file and the packages of its lock.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use serde_json::{Value, json};

/// The Hackage host every package archive comes from.
pub const HACKAGE: &str = "https://hackage.haskell.org";

/// The `(name, version, .cabal sha256)` of a Hackage package mapped to its revision number.
pub type Revisions = BTreeMap<(String, String, String), u64>;

/// Returns the `index-state` that the project file pins.
///
/// # Errors
/// Fails when the project pins none.
pub fn index_state(project: &str) -> Result<String, Error> {
    project
        .lines()
        .filter_map(|line| line.trim().strip_prefix("index-state"))
        .filter_map(|rest| rest.trim_start().strip_prefix(':'))
        .filter_map(|value| value.split_whitespace().next())
        .next()
        .map(str::to_owned)
        .ok_or_else(|| Error::new("the project pins no index-state"))
}

/// Reads the revision number of each wanted `.cabal` file from a Hackage `01-index.tar`.
///
/// # Errors
/// Fails when the archive cannot be read.
pub fn revisions(
    index: impl Read,
    wanted: &BTreeSet<(String, String)>,
) -> Result<Revisions, Error> {
    let fail = |error: std::io::Error| Error::new(format!("01-index.tar: {error}"));
    let paths: BTreeMap<String, &(String, String)> = wanted
        .iter()
        .map(|pair| (format!("{0}/{1}/{0}.cabal", pair.0, pair.1), pair))
        .collect();
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    let mut found = Revisions::new();
    let mut archive = tar::Archive::new(index);
    for entry in archive.entries().map_err(fail)? {
        let mut entry = entry.map_err(fail)?;
        let path = entry.path().map_err(fail)?.to_string_lossy().into_owned();
        let Some((name, version)) = paths.get(&path) else {
            continue;
        };
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(fail)?;
        let ordinal = seen.entry(path).or_insert(0);
        found
            .entry((name.clone(), version.clone(), dx_digest::sha256_hex(&bytes)))
            .or_insert(*ordinal);
        *ordinal += 1;
    }
    Ok(found)
}

/// The `packages:` URLs of a project file mapped to the tarballs they are fetched to.
pub type Fetched = BTreeMap<String, Tarball>;

/// A `packages:` URL fetched and unpacked to a local package directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tarball {
    /// The package directory handed to cabal.
    pub path: PathBuf,
    /// The sha256 of the tarball.
    pub sha256: String,
}

fn is_url(token: &str) -> bool {
    token.starts_with("https://")
}

fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '\t' | ',' | '\r' | '\n')
}

fn map_packages(project: &str, mut each: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(project.len());
    let mut field = false;
    for line in project.split_inclusive('\n') {
        if !line.starts_with([' ', '\t']) {
            field = line.starts_with("packages:");
        }
        if !field {
            out.push_str(line);
            continue;
        }
        let values = line.strip_prefix("packages:").map_or(line, |rest| {
            out.push_str("packages:");
            rest
        });
        let mut rest = values;
        while !rest.is_empty() {
            let end = rest.find(is_separator).unwrap_or(rest.len());
            let (token, tail) = rest.split_at(if end == 0 { 1 } else { end });
            if end == 0 {
                out.push_str(token);
            } else {
                out.push_str(&each(token).unwrap_or_else(|| token.to_owned()));
            }
            rest = tail;
        }
    }
    out
}

/// Returns the URLs that the `packages:` field of a project file names, in order.
#[must_use]
pub fn remote_packages(project: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    map_packages(project, |token| {
        if is_url(token) && !urls.iter().any(|url| url == token) {
            urls.push(token.to_owned());
        }
        None
    });
    urls
}

fn unpack(bytes: &[u8], dir: &Path) -> Result<PathBuf, Error> {
    let fail = |error: std::io::Error| Error::new(format!("{}: {error}", dir.display()));
    tar::Archive::new(flate2::read::GzDecoder::new(bytes))
        .unpack(dir)
        .map_err(fail)?;
    let mut tops = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(fail)? {
        tops.push(entry.map_err(fail)?.path());
    }
    match tops.as_slice() {
        [top] if top.is_dir() => Ok(top.clone()),
        _ => Err(Error::new(format!(
            "{}: the tarball does not hold one top directory",
            dir.display()
        ))),
    }
}

/// Downloads and unpacks the `packages:` URLs of `project` into `dir` and returns the project that names the package directories instead.
///
/// # Errors
/// Fails when the `packages:` field names an `http://` URL, a download fails, a tarball does not match the sha256 that `old` locks for its URL, or a file cannot be written.
pub fn fetch_packages(
    project: &str,
    dir: &Path,
    old: Option<&Lock>,
    download: &dyn Fn(&str) -> Result<Vec<u8>, Error>,
) -> Result<(String, Fetched), Error> {
    let mut plain = None;
    map_packages(project, |token| {
        if plain.is_none() && token.starts_with("http://") {
            plain = Some(token.to_owned());
        }
        None
    });
    if let Some(url) = plain {
        return Err(Error::new(format!("{url}: packages must be https URLs")));
    }
    let mut fetched = Fetched::new();
    for (index, url) in remote_packages(project).into_iter().enumerate() {
        let bytes = download(&url).map_err(|error| Error::new(format!("{url}: {error}")))?;
        let sha256 = dx_digest::sha256_hex(&bytes);
        let locked = old
            .into_iter()
            .flat_map(|lock| &lock.packages)
            .flat_map(|package| &package.archives)
            .find(|archive| archive.urls.contains(&url));
        if let Some(archive) = locked.filter(|archive| archive.sha256 != sha256) {
            return Err(Error::new(format!(
                "{url} has the sha256 {sha256}, the lock has {}",
                archive.sha256
            )));
        }
        let path = unpack(&bytes, &dir.join(index.to_string()))
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        fetched.insert(url, Tarball { path, sha256 });
    }
    let localized = map_packages(project, |token| {
        fetched
            .get(token)
            .map(|tarball| tarball.path.to_string_lossy().into_owned())
    });
    Ok((localized, fetched))
}

fn same_directory(plan: &str, fetched: &Path) -> bool {
    let tail: PathBuf = fetched
        .components()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Path::new(plan.trim_end_matches(['/', '\\'])).ends_with(tail)
}

/// Turns the plan entries of the fetched packages back into entries of their URLs.
pub fn remote_tarballs(plan: &mut Value, fetched: &Fetched) {
    let Some(entries) = plan.get_mut("install-plan").and_then(Value::as_array_mut) else {
        return;
    };
    for entry in entries {
        if entry.pointer("/pkg-src/type").and_then(Value::as_str) != Some("local") {
            continue;
        }
        let path = entry
            .pointer("/pkg-src/path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some((url, tarball)) = fetched
            .iter()
            .find(|(_, tarball)| same_directory(path, &tarball.path))
        else {
            continue;
        };
        if let Some(object) = entry.as_object_mut() {
            object.insert(
                "pkg-src".to_owned(),
                json!({"type": "remote-tar", "uri": url}),
            );
            object.insert("pkg-src-sha256".to_owned(), json!(tarball.sha256));
        }
    }
}

fn text<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn component(entry: &Value) -> &str {
    entry
        .get("component-name")
        .and_then(Value::as_str)
        .unwrap_or("lib")
}

fn source(entry: &Value) -> &str {
    match entry.pointer("/pkg-src/type").and_then(Value::as_str) {
        Some("repo-tar") => "hackage",
        Some("remote-tar") => "url",
        _ if text(entry, "type") == "configured" => "local",
        _ => "ghc",
    }
}

fn dependency(package: &str, component: &str) -> String {
    component.strip_prefix("lib:").map_or_else(
        || package.to_owned(),
        |library| format!("{package}:{library}"),
    )
}

#[derive(Default)]
struct Entry {
    source: String,
    url: String,
    sdist: String,
    cabal: String,
    components: BTreeSet<String>,
    deps: BTreeSet<String>,
    tools: BTreeSet<String>,
}

/// Returns the fetched packages of a cabal `plan.json`, sorted by name.
///
/// # Errors
/// Fails when the plan is malformed or a Hackage package has no revision in `revisions`.
pub fn packages(plan: &Value, revisions: &Revisions) -> Result<Vec<Package>, Error> {
    let entries = plan
        .get("install-plan")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("plan.json has no install-plan"))?;
    let names: BTreeMap<&str, (&str, &str)> = entries
        .iter()
        .map(|entry| {
            (
                text(entry, "id"),
                (text(entry, "pkg-name"), component(entry)),
            )
        })
        .collect();
    let locked: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| matches!(source(entry), "hackage" | "url"))
        .map(|entry| text(entry, "pkg-name"))
        .collect();
    let mut merged: BTreeMap<(String, String), Entry> = BTreeMap::new();
    for entry in entries {
        let name = text(entry, "pkg-name");
        let key = (name.to_owned(), text(entry, "pkg-version").to_owned());
        let package = merged.entry(key).or_default();
        package.source = source(entry).to_owned();
        if package.source == "url" {
            text(entry, "pkg-src-sha256").clone_into(&mut package.sdist);
            entry
                .pointer("/pkg-src/uri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .clone_into(&mut package.url);
        } else if package.source == "hackage" {
            text(entry, "pkg-src-sha256").clone_into(&mut package.sdist);
            text(entry, "pkg-cabal-sha256").clone_into(&mut package.cabal);
        }
        let own = component(entry);
        if !(own == "lib" || own.starts_with("lib:") || own.starts_with("exe:")) {
            continue;
        }
        package.components.insert(own.to_owned());
        let ids = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|id| names.get(id).copied())
                .filter(|(other, _)| *other != name)
                .collect::<Vec<_>>()
        };
        for (other, part) in ids("depends") {
            package.deps.insert(dependency(other, part));
        }
        for (other, part) in ids("exe-depends") {
            if let Some(exe) = part.strip_prefix("exe:") {
                package.tools.insert(format!("{other}:{exe}"));
            }
        }
    }
    let mut out = Vec::new();
    for ((name, version), entry) in merged {
        let id = format!("{name}-{version}");
        let (url, extra_revision) = match entry.source.as_str() {
            "hackage" => {
                let revision = revisions
                    .get(&(name.clone(), version.clone(), entry.cabal.clone()))
                    .ok_or_else(|| {
                        Error::new(format!("no index entry matches the .cabal of {id}"))
                    })?;
                (
                    format!("{HACKAGE}/package/{id}/{id}.tar.gz"),
                    Some(json!({"number": revision, "sha256": entry.cabal})),
                )
            }
            "url" => (entry.url.clone(), None),
            _ => continue,
        };
        let mut extra = BTreeMap::new();
        let deps: BTreeSet<String> = entry
            .deps
            .iter()
            .map(|dep| {
                dep.split_once(':')
                    .map_or(dep.as_str(), |(package, _)| package)
            })
            .filter(|package| locked.contains(package))
            .map(str::to_owned)
            .collect();
        if !entry.deps.is_empty() {
            extra.insert(
                "cabal_deps".to_owned(),
                json!(entry.deps.iter().collect::<Vec<_>>()),
            );
        }
        extra.insert(
            "cabal_components".to_owned(),
            json!(entry.components.iter().collect::<Vec<_>>()),
        );
        if !entry.tools.is_empty() {
            extra.insert(
                "cabal_tools".to_owned(),
                json!(entry.tools.iter().collect::<Vec<_>>()),
            );
        }
        if let Some(revision) = extra_revision {
            extra.insert("cabal_revision".to_owned(), revision);
        }
        out.push(Package {
            name,
            version,
            archives: vec![Archive {
                urls: vec![url],
                sha256: entry.sdist,
                strip_prefix: id,
                ..Archive::default()
            }],
            deps: deps.into_iter().collect(),
            extra,
            ..Package::default()
        });
    }
    Ok(out)
}

/// The cabal lock step: runs the pinned cabal and GHC in the scratch directory.
pub struct Cabal {
    /// The pinned `cabal` executable.
    pub cabal: PathBuf,
    /// The pinned `ghc` executable.
    pub ghc: PathBuf,
}

impl Cabal {
    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }

    fn run(&self, scratch: &Scratch, args: &[&str]) -> Result<(), Error> {
        let bin = self
            .ghc
            .parent()
            .ok_or_else(|| Error::new("ghc has no directory"))?;
        let pkg = bin.join(format!("ghc-pkg{}", std::env::consts::EXE_SUFFIX));
        let output = scratch
            .command(&self.cabal)
            .env("PATH", bin)
            .env("CABAL_DIR", cabal_dir(scratch))
            .arg(format!("--with-compiler={}", self.ghc.display()))
            .arg(format!("--with-hc-pkg={}", pkg.display()))
            .args(args)
            .output()
            .map_err(|error| Error::new(format!("cabal: {error}")))?;
        if output.status.success() {
            return Ok(());
        }
        Err(Error::new(format!(
            "cabal {} failed:\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

impl Resolver for Cabal {
    fn sources(&self) -> Vec<String> {
        vec!["*.cabal".to_owned()]
    }

    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let set = scratch.set();
        let read = |path: PathBuf| {
            std::fs::read(&path).map_err(|error| Error::new(format!("{}: {error}", path.display())))
        };
        let hub =
            dx_families::hub("cabal").ok_or_else(|| Error::new("cabal has no registry entry"))?;
        let project = String::from_utf8_lossy(&read(set.join(&hub.manifest))?).into_owned();
        let state = index_state(&project)?;
        let http = HttpFetch::new();
        let (localized, fetched) = fetch_packages(
            &project,
            &scratch.tmp().with_file_name("packages"),
            old,
            &|url| http.bytes(url),
        )?;
        std::fs::write(set.join(&hub.manifest), localized)
            .map_err(|error| Error::new(format!("{}: {error}", hub.manifest)))?;
        self.run(scratch, &["update", &format!("--index-state={state}")])?;
        self.run(scratch, &["freeze"])?;
        self.run(scratch, &["build", "--dry-run", "all"])?;
        let mut plan: Value =
            serde_json::from_slice(&read(set.join("dist-newstyle/cache/plan.json"))?)
                .map_err(|error| Error::new(format!("plan.json: {error}")))?;
        remote_tarballs(&mut plan, &fetched);
        let index = cabal_dir(scratch).join("packages/hackage.haskell.org/01-index.tar");
        let file = std::fs::File::open(&index)
            .map_err(|error| Error::new(format!("{}: {error}", index.display())))?;
        let found = revisions(file, &hackage_pairs(&plan))?;
        std::fs::remove_dir_all(set.join("dist-newstyle"))
            .map_err(|error| Error::new(format!("dist-newstyle: {error}")))?;
        Ok(Resolution {
            packages: packages(&plan, &found)?,
        })
    }
}

fn cabal_dir(scratch: &Scratch) -> PathBuf {
    scratch.tmp().with_file_name("cabal")
}

fn hackage_pairs(plan: &Value) -> BTreeSet<(String, String)> {
    plan.get("install-plan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| source(entry) == "hackage")
        .map(|entry| {
            (
                text(entry, "pkg-name").to_owned(),
                text(entry, "pkg-version").to_owned(),
            )
        })
        .collect()
}
