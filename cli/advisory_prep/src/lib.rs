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
#[path = "trivy.rs"]
pub mod trivy;
#[path = "yanked.rs"]
pub mod yanked;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use dx_atomic_fs::write_atomic;
use dx_audit::advisory_prep::{
    convert_entries, cpan_snapshot, fetch_many_plan, identity_bytes, needed_families,
    split_cpansa_output, FetchLimits, PrepError, DEFAULT_LIMITS,
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
    /// File holding the cpansa_dump output instead of running the binary.
    pub cpansa_output: Option<PathBuf>,
    /// The `//perl/tools:cpansa_dump` binary that prints the CPANSA::DB database.
    pub cpansa_dump: Option<PathBuf>,
    /// The pinned trivy binary; set, it downloads the trivy databases under `out`.
    pub trivy: Option<PathBuf>,
    /// Base URL of the cargo sparse index the yanked flags are read from.
    pub cargo_index: Option<String>,
    /// Base URL of the PyPI JSON API the yanked flags are read from.
    pub pypi_index: Option<String>,
}

impl Default for PrepareOptions {
    fn default() -> Self {
        PrepareOptions {
            workspace: PathBuf::from("."),
            out: PathBuf::from(DEFAULT_OUT),
            retrieved_at: String::new(),
            limits: DEFAULT_LIMITS,
            archives: BTreeMap::new(),
            cpansa_output: None,
            cpansa_dump: None,
            trivy: None,
            cargo_index: None,
            pypi_index: None,
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

/// Outcome of one preparation run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    pub status: &'static str,
    pub ecosystems: usize,
    pub families: Vec<String>,
}

impl Summary {
    /// Builds the summary of the snapshots one run published.
    pub fn of(prepared: &[Prepared]) -> Self {
        let prepared: Vec<&Prepared> = prepared
            .iter()
            .filter(|entry| !entry.family.ends_with("-yanked"))
            .collect();
        Summary {
            status: if prepared.is_empty() {
                "empty"
            } else {
                "prepared"
            },
            ecosystems: prepared.len(),
            families: prepared.iter().map(|entry| entry.family.clone()).collect(),
        }
    }

    /// Renders the key=value lines a workflow step output takes.
    pub fn render(&self) -> String {
        format!(
            "status={}\necosystems={}\nfamilies={}\n",
            self.status,
            self.ecosystems,
            self.families.join(",")
        )
    }
}

/// The families and upstream archives one preparation run will convert.
pub fn plan(options: &PrepareOptions) -> Result<Vec<(&'static str, String)>, PrepError> {
    let mut families = needed_families(&options.workspace)?;
    families.extend(configured_families(&options.workspace)?);
    families.sort_unstable();
    families.dedup();
    let mut out = Vec::new();
    for family in families {
        let url = dx_audit::advisory::advisory_source(family).ok_or_else(|| {
            PrepError::UnsupportedSet {
                set: family.to_owned(),
            }
        })?;
        out.push((family, url.to_owned()));
    }
    Ok(out)
}

fn configured_families(workspace: &Path) -> Result<Vec<&'static str>, PrepError> {
    let registry =
        dx_adopt::dependency_sets::load(workspace).map_err(|error| PrepError::Usage {
            detail: error.to_string(),
        })?;
    Ok(registry
        .map(|registry| {
            registry
                .sets
                .iter()
                .filter(|set| set.locks.iter().any(|rel| workspace.join(rel).is_file()))
                .filter_map(|set| set.ecosystem.advisory_family())
                .collect()
        })
        .unwrap_or_default())
}

/// Prepares every snapshot the workspace's dependency sets need, once.
pub fn prepare(options: &PrepareOptions) -> Result<Vec<Prepared>, PrepError> {
    let mut out = Vec::new();
    for (family, url) in plan(options)? {
        out.push(prepare_family(options, family, &url)?);
        if family == "cargo" {
            let prepared = yanked::prepare_cargo_yanked(options)?;
            out.push(publish(options, yanked::CARGO_YANKED, prepared)?);
        }
        if family == dx_audit::advisory::PYPI_FAMILY {
            let prepared = yanked::prepare_pypi_yanked(options)?;
            out.push(publish(options, yanked::PYPI_YANKED, prepared)?);
        }
    }
    if let Some(binary) = &options.trivy {
        trivy::prepare_trivy(binary, &options.out, &options.retrieved_at)?;
    }
    Ok(out)
}

fn prepare_family(
    options: &PrepareOptions,
    family: &str,
    url: &str,
) -> Result<Prepared, PrepError> {
    let prepared = if family == "cpan" {
        prepare_cpan(options)?
    } else {
        let archive = read_archive(options, family, url)?;
        let entries = archive::json_entries(&archive).map_err(|error| PrepError::Archive {
            family: family.to_owned(),
            detail: error.to_string(),
        })?;
        convert_entries(family, url, &options.retrieved_at, &entries)?
    };
    publish(options, family, prepared)
}

fn publish(
    options: &PrepareOptions,
    family: &str,
    prepared: dx_audit::advisory_prep::PreparedSnapshot,
) -> Result<Prepared, PrepError> {
    let identity = identity_bytes(&prepared.snapshot)?;
    let meta_path = options.out.join(format!("{family}.meta.json"));
    match std::fs::remove_file(&meta_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(PrepError::Write {
                path: dx_audit::advisory::identity_rel(family),
                detail: error.to_string(),
            })
        }
    }
    write_atomic(
        &options.out.join(format!("{family}.json")),
        &prepared.payload,
    )
    .map_err(|error| PrepError::Write {
        path: prepared.snapshot.path.clone(),
        detail: error.to_string(),
    })?;
    let meta_rel = dx_audit::advisory::identity_rel(family);
    dx_atomic_fs::write_atomic(&meta_path, &identity).map_err(|error| PrepError::Write {
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

fn prepare_cpan(
    options: &PrepareOptions,
) -> Result<dx_audit::advisory_prep::PreparedSnapshot, PrepError> {
    let output = match (&options.cpansa_output, &options.cpansa_dump) {
        (Some(path), _) => {
            std::fs::read_to_string(path).map_err(|error| PrepError::ArchiveRead {
                family: "cpan".to_owned(),
                path: path.display().to_string(),
                detail: error.to_string(),
            })?
        }
        (None, Some(binary)) => run_cpansa_dump(binary)?,
        (None, None) => {
            return Err(PrepError::Usage {
                detail: "the cpan family needs --cpansa-dump or --cpansa-output".to_owned(),
            })
        }
    };
    let (version, database) = split_cpansa_output(&output)?;
    cpan_snapshot(database, version, &options.retrieved_at)
}

fn run_cpansa_dump(binary: &Path) -> Result<String, PrepError> {
    let program = binary.display().to_string();
    let done = Process::new(binary)
        .output()
        .map_err(|error| PrepError::Fetch {
            program: program.clone(),
            detail: error.to_string(),
        })?;
    if !done.status.success() {
        return Err(PrepError::Fetch {
            program,
            detail: format!(
                "exited with {}: {}",
                exit_text(done.status),
                String::from_utf8_lossy(&done.stderr).trim()
            ),
        });
    }
    String::from_utf8(done.stdout).map_err(|error| PrepError::Fetch {
        program,
        detail: error.to_string(),
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
    let scratch = tempfile::Builder::new()
        .prefix("dx-advisory-")
        .tempdir()
        .map_err(|error| PrepError::ArchiveRead {
            family: family.to_owned(),
            path: std::env::temp_dir().display().to_string(),
            detail: error.to_string(),
        })?;
    let destination = scratch.path().join(format!("{family}.zip"));
    fetch(url, &destination, options.limits)?;
    std::fs::read(&destination).map_err(|error| PrepError::ArchiveRead {
        family: family.to_owned(),
        path: destination.display().to_string(),
        detail: error.to_string(),
    })
}

/// Runs the bounded download that fetches one snapshot archive.
pub fn fetch(url: &str, destination: &Path, limits: FetchLimits) -> Result<(), PrepError> {
    fetch_many(&[(url.to_owned(), destination.to_path_buf())], limits)
}

/// Most transfers one parallel curl run keeps open at once.
pub const PARALLEL_MAX: u32 = 16;

/// Runs one bounded parallel curl that fetches every file.
pub fn fetch_many(downloads: &[(String, PathBuf)], limits: FetchLimits) -> Result<(), PrepError> {
    if downloads.is_empty() {
        return Ok(());
    }
    let plan = fetch_many_plan(downloads, limits, PARALLEL_MAX)?;
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
            detail: format!("{} files exited with {}", downloads.len(), exit_text(done)),
        });
    }
    if let Some((url, _)) = downloads.iter().find(|(_, file)| !file.is_file()) {
        return Err(PrepError::Fetch {
            program: plan.program,
            detail: format!("{url} was not written"),
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
