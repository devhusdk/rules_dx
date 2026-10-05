#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

#[path = "archive.rs"]
pub mod archive;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use dx_atomic_fs::write_atomic;
use dx_audit::advisory_prep::{
    convert_entries, fetch_plan, identity_bytes, needed_families, FetchLimits, PrepError,
    DEFAULT_LIMITS,
};

pub const DEFAULT_OUT: &str = ".dx/advisory";

/// How one preparation run reaches and writes its snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareOptions {
    pub workspace: PathBuf,
    pub out: PathBuf,
    pub retrieved_at: String,
    pub limits: FetchLimits,
    pub archives: BTreeMap<String, PathBuf>,
}

impl Default for PrepareOptions {
    fn default() -> Self {
        PrepareOptions {
            workspace: PathBuf::from("."),
            out: PathBuf::from(DEFAULT_OUT),
            retrieved_at: String::new(),
            limits: DEFAULT_LIMITS,
            archives: BTreeMap::new(),
        }
    }
}

/// One snapshot one preparation run published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prepared {
    pub family: String,
    pub path: String,
    pub identity: String,
    pub advisories: usize,
}

/// The families and upstream archives one preparation run will convert.
pub fn plan(options: &PrepareOptions) -> Result<Vec<(&'static str, String)>, PrepError> {
    let mut out = Vec::new();
    for family in needed_families(&options.workspace)? {
        let url = dx_audit::advisory::advisory_source(family).ok_or_else(|| {
            PrepError::UnsupportedSet {
                set: family.to_owned(),
            }
        })?;
        out.push((family, url.to_owned()));
    }
    Ok(out)
}

/// Prepares every snapshot the workspace's dependency sets need, once.
pub fn prepare(options: &PrepareOptions) -> Result<Vec<Prepared>, PrepError> {
    let mut out = Vec::new();
    for (family, url) in plan(options)? {
        out.push(prepare_family(options, family, &url)?);
    }
    Ok(out)
}

fn prepare_family(
    options: &PrepareOptions,
    family: &str,
    url: &str,
) -> Result<Prepared, PrepError> {
    let archive = read_archive(options, family, url)?;
    let entries = archive::json_entries(&archive).map_err(|error| PrepError::Archive {
        family: family.to_owned(),
        detail: error.to_string(),
    })?;
    let prepared = convert_entries(family, url, &options.retrieved_at, &entries)?;
    let identity = identity_bytes(&prepared.snapshot)?;
    write_atomic(
        &options.out.join(format!("{family}.json")),
        &prepared.payload,
    )
    .map_err(|error| PrepError::Write {
        path: prepared.snapshot.path.clone(),
        detail: error.to_string(),
    })?;
    let meta_rel = dx_audit::advisory::identity_rel(family);
    dx_atomic_fs::write_atomic(&options.out.join(format!("{family}.meta.json")), &identity)
        .map_err(|error| PrepError::Write {
            path: meta_rel,
            detail: error.to_string(),
        })?;
    Ok(Prepared {
        family: family.to_owned(),
        path: prepared.snapshot.path,
        identity: prepared.snapshot.sha256,
        advisories: prepared.advisories,
    })
}

fn read_archive(options: &PrepareOptions, family: &str, url: &str) -> Result<Vec<u8>, PrepError> {
    if let Some(path) = options.archives.get(family) {
        return std::fs::read(path).map_err(|error| PrepError::ArchiveRead {
            family: family.to_owned(),
            path: path.display().to_string(),
            detail: error.to_string(),
        });
    }
    let destination = std::env::temp_dir().join(format!("dx-advisory-{family}.zip"));
    fetch(url, &destination, options.limits)?;
    let archive = std::fs::read(&destination).map_err(|error| PrepError::ArchiveRead {
        family: family.to_owned(),
        path: destination.display().to_string(),
        detail: error.to_string(),
    });
    let _ = std::fs::remove_file(&destination);
    archive
}

/// Runs the bounded download that fetches one snapshot archive.
pub fn fetch(url: &str, destination: &Path, limits: FetchLimits) -> Result<(), PrepError> {
    let plan = fetch_plan(url, destination, limits)?;
    let done = Process::new(&plan.program)
        .args(&plan.argv)
        .status()
        .map_err(|error| PrepError::Fetch {
            program: plan.program.clone(),
            detail: error.to_string(),
        })?;
    if !done.success() {
        return Err(PrepError::Fetch {
            program: plan.program,
            detail: format!("{url} exited with {}", exit_text(done)),
        });
    }
    Ok(())
}

fn exit_text(done: std::process::ExitStatus) -> String {
    match done.code() {
        Some(code) => format!("status {code}"),
        None => "a signal".to_owned(),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
