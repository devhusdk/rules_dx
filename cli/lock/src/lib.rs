#![deny(clippy::indexing_slicing, clippy::string_slice)]

//! The lock step of a dx class ecosystem and the only writer of `<ecosystem>.lock.json`.

mod http;
mod model;
mod scratch;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use globset::{Glob, GlobSetBuilder};

pub use http::HttpFetch;
pub use model::{Archive, Lock, PLATFORMS, Package, Resolution, SCHEMA, is_sha256};
pub use scratch::Scratch;

/// Why a lock step failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    /// An error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// The registry values of one ecosystem that the lock step reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spec {
    /// The `ecosystem` value.
    pub ecosystem: String,
    /// The `manifest` names, best first; each is a file name glob such as `*.opam`.
    pub manifest: Vec<String>,
    /// The `state` names, best first; empty when the resolver keeps no state.
    pub state: Vec<String>,
}

impl Spec {
    /// The lock file name.
    #[must_use]
    pub fn lock_name(&self) -> String {
        format!("{}.lock.json", self.ecosystem)
    }
}

/// Resolves the closure of one dependency set.
pub trait Resolver {
    /// Resolves the manifest in `scratch.set()`, writes the state file there, and returns the closure.
    ///
    /// `old` is the lock of the previous run, when it has one for this ecosystem.
    ///
    /// # Errors
    /// Fails when the manifest has no resolution.
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error>;

    /// The names of the further files the resolver reads; each is a file name glob such as `*.cabal`.
    ///
    /// Every file that any of them matches is copied into `scratch.set()` and digested with the manifest.
    fn sources(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Reads archives the lock has no digest for.
pub trait Fetch {
    /// Downloads `url` and returns the sha256 of its bytes.
    ///
    /// # Errors
    /// Fails when the download does.
    fn sha256(&self, url: &str) -> Result<String, Error>;
}

/// How a lock step ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The lock matches its inputs; nothing ran.
    Fresh,
    /// The lock was resolved and written.
    Written,
    /// `--check` found the lock missing or stale.
    Stale,
}

/// The parsed flags of a lock step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Args {
    /// The set directory.
    pub dir: PathBuf,
    /// Whether to compare only.
    pub check: bool,
    /// The packages to resolve afresh instead of keeping their locked version.
    pub upgrade: Vec<String>,
}

/// The flags, one line each.
pub const USAGE: &str = "\
--dir <path>  the absolute directory of the dependency set
--check       compare only; exit 1 when the lock is stale
--upgrade <name>  resolve this package afresh; repeatable";

impl Args {
    /// Parses `--dir <path>`, `--check` and `--upgrade <name>`.
    ///
    /// # Errors
    /// Fails on an unknown flag and on a missing or relative `--dir`.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, Error> {
        let mut dir = None;
        let mut check = false;
        let mut upgrade = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--check" => check = true,
                "--dir" => dir = args.next(),
                "--upgrade" => upgrade.extend(args.next()),
                other => match (
                    other.strip_prefix("--dir="),
                    other.strip_prefix("--upgrade="),
                ) {
                    (Some(value), _) => dir = Some(value.to_owned()),
                    (_, Some(value)) => upgrade.push(value.to_owned()),
                    _ => return Err(Error::new(format!("unknown argument {other}\n{USAGE}"))),
                },
            }
        }
        let dir =
            PathBuf::from(dir.ok_or_else(|| Error::new(format!("--dir is missing\n{USAGE}")))?);
        if !dir.is_absolute() {
            return Err(Error::new(format!(
                "--dir {} is not absolute\n{USAGE}",
                dir.display()
            )));
        }
        Ok(Self {
            dir,
            check,
            upgrade,
        })
    }
}

/// Parses `pattern`, a file name glob such as `*.opam`.
fn glob(pattern: &str) -> Result<Glob, Error> {
    Glob::new(pattern).map_err(|error| Error::new(format!("file pattern {pattern}: {error}")))
}

/// The sorted names of the files in `dir`.
fn file_names(dir: &Path) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|error| Error::new(format!("{}: {error}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::new(format!("{}: {error}", dir.display())))?;
        if entry.path().is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// The files of `dir` named by the first of `patterns` that names any, sorted.
fn named_files(dir: &Path, patterns: &[String]) -> Result<Vec<String>, Error> {
    let names = file_names(dir)?;
    for pattern in patterns {
        let glob = glob(pattern)?.compile_matcher();
        let found: Vec<String> = names
            .iter()
            .filter(|name| glob.is_match(name))
            .cloned()
            .collect();
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(Vec::new())
}

/// The files of `dir` named by any of `patterns`, sorted.
fn all_named_files(dir: &Path, patterns: &[String]) -> Result<Vec<String>, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(glob(pattern)?);
    }
    let globs = builder
        .build()
        .map_err(|error| Error::new(format!("file patterns {patterns:?}: {error}")))?;
    Ok(file_names(dir)?
        .into_iter()
        .filter(|name| globs.is_match(name))
        .collect())
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|error| Error::new(format!("{}: {error}", path.display())))
}

/// Writes `bytes` to `path` through a sibling file, so a reader sees the old or the new file.
fn write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let failed = |error: std::io::Error| Error::new(format!("{}: {error}", path.display()));
    let mut sibling = path.as_os_str().to_owned();
    sibling.push(format!(".{}.tmp", std::process::id()));
    let sibling = PathBuf::from(sibling);
    std::fs::write(&sibling, bytes).map_err(failed)?;
    std::fs::rename(&sibling, path).map_err(|error| {
        drop(std::fs::remove_file(&sibling));
        failed(error)
    })
}

/// The manifest and state files of a set.
struct Inputs {
    manifest: Vec<String>,
    sources: Vec<String>,
    state: Vec<String>,
}

impl Inputs {
    fn of(spec: &Spec, sources: &[String], dir: &Path) -> Result<Self, Error> {
        let manifest = named_files(dir, &spec.manifest)?;
        if manifest.is_empty() {
            return Err(Error::new(format!(
                "{} has no {}",
                dir.display(),
                spec.manifest.join(" or ")
            )));
        }
        Ok(Self {
            sources: all_named_files(dir, sources)?
                .into_iter()
                .filter(|name| !manifest.contains(name))
                .collect(),
            manifest,
            state: named_files(dir, &spec.state)?,
        })
    }

    fn digests(&self, dir: &Path) -> Result<BTreeMap<String, String>, Error> {
        let mut digests = BTreeMap::new();
        for name in self.copied() {
            digests.insert(name.clone(), dx_digest::sha256_hex(&read(&dir.join(name))?));
        }
        Ok(digests)
    }

    fn copied(&self) -> impl Iterator<Item = &String> {
        self.manifest.iter().chain(&self.sources).chain(&self.state)
    }
}

/// The digest of `wanted` in `old` when the package name, version and archive URLs are the same.
fn kept<'a>(old: Option<&'a [Package]>, package: &Package, wanted: &Archive) -> Option<&'a str> {
    old?.iter()
        .find(|entry| entry.name == package.name && entry.version == package.version)?
        .archives
        .iter()
        .find(|archive| archive.urls == wanted.urls)
        .map(|archive| archive.sha256.as_str())
}

/// Fills every empty archive digest of `packages` from `old`, else from a download.
fn fill_digests(
    packages: &mut [Package],
    old: Option<&[Package]>,
    fetch: &dyn Fetch,
) -> Result<(), Error> {
    for entry in packages.iter_mut() {
        let snapshot = entry.clone();
        for wanted in &mut entry.archives {
            if !wanted.sha256.is_empty() {
                continue;
            }
            wanted.sha256 = match kept(old, &snapshot, wanted) {
                Some(digest) => digest.to_owned(),
                None => fetched(fetch, &snapshot, wanted)?,
            };
        }
    }
    Ok(())
}

fn fetched(fetch: &dyn Fetch, package: &Package, archive: &Archive) -> Result<String, Error> {
    let mut failures = Vec::new();
    for url in &archive.urls {
        match fetch.sha256(url) {
            Ok(digest) if is_sha256(&digest) => return Ok(digest),
            Ok(digest) => failures.push(format!("{url}: got the digest {digest:?}")),
            Err(error) => failures.push(format!("{url}: {error}")),
        }
    }
    Err(Error::new(format!(
        "package {} {} has no archive to read:\n{}",
        package.name,
        package.version,
        failures.join("\n")
    )))
}

/// Runs the lock step of one set.
///
/// # Errors
/// Fails when the set has no manifest, the resolver or a download fails, or a file cannot be written.
pub fn run(
    spec: &Spec,
    args: &Args,
    resolver: &dyn Resolver,
    fetch: &dyn Fetch,
) -> Result<Outcome, Error> {
    let dir = &args.dir;
    let lock_path = dir.join(spec.lock_name());
    let inputs = Inputs::of(spec, &resolver.sources(), dir)?;
    let old_text = std::fs::read_to_string(&lock_path).ok();
    let old = old_text
        .as_deref()
        .and_then(|text| Lock::from_json(text).ok())
        .filter(|lock| lock.ecosystem == spec.ecosystem);
    if args.upgrade.is_empty()
        && old
            .as_ref()
            .is_some_and(|lock| inputs.digests(dir).is_ok_and(|now| now == lock.inputs))
    {
        return Ok(Outcome::Fresh);
    }
    if args.check {
        return Ok(Outcome::Stale);
    }
    let scratch = Scratch::new()?;
    for name in inputs.copied() {
        std::fs::write(scratch.set().join(name), read(&dir.join(name))?)
            .map_err(|error| Error::new(format!("{name}: {error}")))?;
    }
    let kept_packages = |packages: &[Package]| -> Vec<Package> {
        packages
            .iter()
            .filter(|package| !args.upgrade.contains(&package.name))
            .cloned()
            .collect()
    };
    let seed = old.as_ref().map(|lock| Lock {
        packages: kept_packages(&lock.packages),
        groups: lock
            .groups
            .iter()
            .map(|(group, packages)| (group.clone(), kept_packages(packages)))
            .collect(),
        ..lock.clone()
    });
    let resolution = resolver.resolve(&scratch, seed.as_ref())?;
    let mut lock = Lock {
        schema: SCHEMA,
        ecosystem: spec.ecosystem.clone(),
        inputs: BTreeMap::new(),
        packages: resolution.packages,
        groups: resolution.groups,
    };
    lock.normalize();
    let problems = lock.shape_problems();
    if !problems.is_empty() {
        return Err(Error::new(format!(
            "the resolver returned an unusable closure:\n{}",
            problems.join("\n")
        )));
    }
    fill_digests(
        &mut lock.packages,
        old.as_ref().map(|old| old.packages.as_slice()),
        fetch,
    )?;
    for (group, packages) in &mut lock.groups {
        let previous = old
            .as_ref()
            .and_then(|old| old.groups.get(group))
            .map(Vec::as_slice);
        fill_digests(packages, previous, fetch)?;
    }
    let state = named_files(scratch.set(), &spec.state)?;
    for name in &state {
        let bytes = read(&scratch.set().join(name))?;
        if std::fs::read(dir.join(name)).ok().as_ref() != Some(&bytes) {
            write(&dir.join(name), &bytes)?;
        }
    }
    lock.inputs = Inputs {
        manifest: inputs.manifest,
        sources: inputs.sources,
        state,
    }
    .digests(dir)?;
    let text = lock.to_bytes();
    if old_text.as_deref() != Some(text.as_str()) {
        write(&lock_path, text.as_bytes())?;
    }
    Ok(Outcome::Written)
}

/// The `main` of an ecosystem's resolver target: exit 0 when the lock is fresh or written, 1 when stale, 2 on failure.
pub fn main(spec: &Spec, resolver: &dyn Resolver) -> ExitCode {
    let outcome = Args::parse(std::env::args().skip(1)).and_then(|args| {
        run(spec, &args, resolver, &HttpFetch::new()).map(|outcome| (outcome, args))
    });
    match outcome {
        Ok((Outcome::Stale, args)) => {
            eprintln!(
                "{} is stale; run `dx generate --apply`",
                args.dir.join(spec.lock_name()).display()
            );
            ExitCode::from(1)
        }
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: {error}", spec.lock_name());
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory holding empty files with `names`.
    fn dir_with(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in names {
            std::fs::write(dir.path().join(name), "").expect("write");
        }
        dir
    }

    fn patterns(patterns: &[&str]) -> Vec<String> {
        patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect()
    }

    #[test]
    fn the_first_pattern_that_names_a_file_wins() {
        let dir = dir_with(&["b.opam", "a.opam", "a.opam.locked", "dune-project"]);
        assert_eq!(
            named_files(dir.path(), &patterns(&["missing.json", "*.opam", "*"])),
            Ok(vec!["a.opam".to_owned(), "b.opam".to_owned()])
        );
        assert_eq!(
            named_files(dir.path(), &patterns(&["*.json"])),
            Ok(Vec::new())
        );
    }

    #[test]
    fn every_pattern_adds_its_files() {
        let dir = dir_with(&["a.cabal", "b.cabal", "stack.yaml.lock", "README.md"]);
        assert_eq!(
            all_named_files(dir.path(), &patterns(&["*.cabal", "stack.yaml.lock"])),
            Ok(vec![
                "a.cabal".to_owned(),
                "b.cabal".to_owned(),
                "stack.yaml.lock".to_owned()
            ])
        );
    }

    #[test]
    fn many_stars_match_in_linear_time() {
        // A backtracking matcher tries every split of the name per `*`; it would not finish here.
        let name = "a".repeat(64);
        let dir = dir_with(&[name.as_str()]);
        let no_match = format!("{}b", "*a".repeat(16));
        assert_eq!(named_files(dir.path(), &[no_match]), Ok(Vec::new()));
        let all = format!("{}*", "*a".repeat(16));
        assert_eq!(named_files(dir.path(), &[all]), Ok(vec![name]));
    }

    #[test]
    fn a_broken_pattern_is_an_error() {
        let dir = dir_with(&["a.opam"]);
        let error = named_files(dir.path(), &patterns(&["[.opam"]))
            .expect_err("an unclosed class is no glob");
        assert!(
            error.to_string().starts_with("file pattern [.opam: "),
            "{error}"
        );
        assert!(all_named_files(dir.path(), &patterns(&["[.opam"])).is_err());
    }
}
