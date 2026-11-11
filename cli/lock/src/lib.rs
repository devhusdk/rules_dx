#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The lock step of a dx class ecosystem and the only writer of `<ecosystem>.lock.json`.

mod http;
mod model;
mod scratch;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
    /// The `manifest` names, best first; `*` matches any run of characters.
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

/// Whether `name` matches `pattern`, where `*` is any run of characters.
fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => name.strip_prefix(head).is_some_and(|rest| {
            (0..=rest.len()).any(|at| rest.is_char_boundary(at) && matches(tail, &rest[at..]))
        }),
    }
}

/// The files of `dir` named by the first of `patterns` that names any, sorted.
fn named_files(dir: &Path, patterns: &[String]) -> Result<Vec<String>, Error> {
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
    for pattern in patterns {
        let found: Vec<String> = names
            .iter()
            .filter(|name| matches(pattern, name))
            .cloned()
            .collect();
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(Vec::new())
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
        let _ = std::fs::remove_file(&sibling);
        failed(error)
    })
}

/// The manifest and state files of a set.
struct Inputs {
    manifest: Vec<String>,
    state: Vec<String>,
}

impl Inputs {
    fn of(spec: &Spec, dir: &Path) -> Result<Self, Error> {
        let manifest = named_files(dir, &spec.manifest)?;
        if manifest.is_empty() {
            return Err(Error::new(format!(
                "{} has no {}",
                dir.display(),
                spec.manifest.join(" or ")
            )));
        }
        Ok(Self {
            manifest,
            state: named_files(dir, &spec.state)?,
        })
    }

    fn digests(&self, dir: &Path) -> Result<BTreeMap<String, String>, Error> {
        let mut digests = BTreeMap::new();
        for name in self.manifest.iter().chain(&self.state) {
            digests.insert(name.clone(), dx_digest::sha256_hex(&read(&dir.join(name))?));
        }
        Ok(digests)
    }
}

/// The digest of `wanted` in `old` when the package name, version and archive URLs are the same.
fn kept<'a>(old: Option<&'a Lock>, package: &Package, wanted: &Archive) -> Option<&'a str> {
    old?.packages
        .iter()
        .find(|entry| entry.name == package.name && entry.version == package.version)?
        .archives
        .iter()
        .find(|archive| archive.urls == wanted.urls)
        .map(|archive| archive.sha256.as_str())
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
    let inputs = Inputs::of(spec, dir)?;
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
    for name in inputs.manifest.iter().chain(&inputs.state) {
        std::fs::write(scratch.set().join(name), read(&dir.join(name))?)
            .map_err(|error| Error::new(format!("{name}: {error}")))?;
    }
    let seed = old.as_ref().map(|lock| Lock {
        packages: lock
            .packages
            .iter()
            .filter(|package| !args.upgrade.contains(&package.name))
            .cloned()
            .collect(),
        ..lock.clone()
    });
    let mut lock = Lock {
        schema: SCHEMA,
        ecosystem: spec.ecosystem.clone(),
        inputs: BTreeMap::new(),
        packages: resolver.resolve(&scratch, seed.as_ref())?.packages,
    };
    lock.normalize();
    let problems = lock.shape_problems();
    if !problems.is_empty() {
        return Err(Error::new(format!(
            "the resolver returned an unusable closure:\n{}",
            problems.join("\n")
        )));
    }
    for package in 0..lock.packages.len() {
        for archive in 0..lock.packages[package].archives.len() {
            let (entry, wanted) = (
                &lock.packages[package],
                &lock.packages[package].archives[archive],
            );
            if !wanted.sha256.is_empty() {
                continue;
            }
            let digest = match kept(old.as_ref(), entry, wanted) {
                Some(digest) => digest.to_owned(),
                None => fetched(fetch, entry, wanted)?,
            };
            lock.packages[package].archives[archive].sha256 = digest;
        }
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
