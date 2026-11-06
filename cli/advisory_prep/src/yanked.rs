use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_adopt::dependency_sets::{Ecosystem, CARGO_LOCK};
use dx_audit::advisory_prep::{yanked_snapshot, PrepError, PreparedSnapshot};
use dx_update::discover::cargo::{index_path, yanked_versions, DEFAULT_BASE};
use dx_update::discover::pypi::{yanked_releases, DEFAULT_BASE as PYPI_BASE};

use crate::{fetch_many, PrepareOptions};

pub const CARGO_YANKED: &str = "cargo-yanked";
const UV_LOCK: &str = "uv.lock";
pub const PYPI_YANKED: &str = "pypi-yanked";

/// Where the cargo index is read from: the options value or the crates.io sparse index.
pub fn index_base(options: &PrepareOptions) -> &str {
    options.cargo_index.as_deref().unwrap_or(DEFAULT_BASE)
}

/// Builds the yanked-version snapshot for the cargo locks of the workspace.
pub fn prepare_cargo_yanked(options: &PrepareOptions) -> Result<PreparedSnapshot, PrepError> {
    let base = index_base(options).trim_end_matches('/').to_owned();
    let locked = locked_registry_crates(&options.workspace)?;
    let urls: Vec<String> = locked
        .keys()
        .map(|name| format!("{base}/{}", index_path(name)))
        .collect();
    let bodies = fetch_bodies(CARGO_YANKED, &urls, options)?;
    let mut yanked = Vec::new();
    for (((name, versions), url), body) in locked.iter().zip(&urls).zip(&bodies) {
        let listed = yanked_versions(body).map_err(|detail| PrepError::Usage {
            detail: format!("{url}: {detail}"),
        })?;
        for version in versions.iter().filter(|version| listed.contains(version)) {
            yanked.push((name.clone(), version.clone()));
        }
    }
    yanked_snapshot(CARGO_YANKED, &base, &options.retrieved_at, "cargo", &yanked)
}

/// Where the PyPI JSON API is read from: the options value or pypi.org.
pub fn pypi_base(options: &PrepareOptions) -> &str {
    options.pypi_index.as_deref().unwrap_or(PYPI_BASE)
}

/// Builds the yanked-version snapshot for the uv locks of the workspace.
pub fn prepare_pypi_yanked(options: &PrepareOptions) -> Result<PreparedSnapshot, PrepError> {
    let base = pypi_base(options).trim_end_matches('/').to_owned();
    let locked = locked_pypi_packages(&options.workspace)?;
    let urls: Vec<String> = locked
        .keys()
        .map(|name| format!("{base}/{name}/json"))
        .collect();
    let bodies = fetch_bodies(PYPI_YANKED, &urls, options)?;
    let mut yanked = Vec::new();
    for (((name, versions), url), body) in locked.iter().zip(&urls).zip(&bodies) {
        let listed = yanked_releases(body).map_err(|detail| PrepError::Usage {
            detail: format!("{url}: {detail}"),
        })?;
        for version in versions.iter().filter(|version| listed.contains(version)) {
            yanked.push((name.clone(), version.clone()));
        }
    }
    yanked_snapshot(PYPI_YANKED, &base, &options.retrieved_at, "uv", &yanked)
}

fn fetch_bodies(
    family: &str,
    urls: &[String],
    options: &PrepareOptions,
) -> Result<Vec<String>, PrepError> {
    let scratch = tempfile::Builder::new()
        .prefix("dx-yanked-")
        .tempdir()
        .map_err(|error| PrepError::ArchiveRead {
            family: family.to_owned(),
            path: std::env::temp_dir().display().to_string(),
            detail: error.to_string(),
        })?;
    let downloads: Vec<(String, std::path::PathBuf)> = urls
        .iter()
        .enumerate()
        .map(|(index, url)| (url.clone(), scratch.path().join(format!("{index}.json"))))
        .collect();
    fetch_many(&downloads, options.limits)?;
    downloads
        .iter()
        .map(|(url, file)| {
            std::fs::read_to_string(file).map_err(|error| PrepError::ArchiveRead {
                family: family.to_owned(),
                path: url.clone(),
                detail: error.to_string(),
            })
        })
        .collect()
}

/// Normalized package name to its locked versions, for packages from the PyPI index only.
fn locked_pypi_packages(workspace: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, PrepError> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for rel in set_locks(workspace, Ecosystem::Uv, UV_LOCK)? {
        let Ok(text) = std::fs::read_to_string(workspace.join(&rel)) else {
            continue;
        };
        let Ok(packages) = dx_audit::locks::parse_uv_lock(&text) else {
            continue;
        };
        for package in packages
            .into_iter()
            .filter(|package| !package.is_git && !package.is_private)
        {
            out.entry(dx_audit::vuln::normalize_pypi_name(&package.name))
                .or_default()
                .insert(package.version);
        }
    }
    Ok(out)
}

/// Crate name to its locked versions, for registry crates only.
fn locked_registry_crates(
    workspace: &Path,
) -> Result<std::collections::BTreeMap<String, BTreeSet<String>>, PrepError> {
    let mut out: std::collections::BTreeMap<String, BTreeSet<String>> = Default::default();
    for rel in cargo_locks(workspace)? {
        let Ok(text) = std::fs::read_to_string(workspace.join(&rel)) else {
            continue;
        };
        let Ok(packages) = dx_audit::locks::parse_cargo_lock(&text) else {
            continue;
        };
        for package in packages
            .into_iter()
            .filter(|package| !package.is_git && !package.is_private)
        {
            out.entry(package.name).or_default().insert(package.version);
        }
    }
    Ok(out)
}

fn cargo_locks(workspace: &Path) -> Result<BTreeSet<String>, PrepError> {
    let mut locks: BTreeSet<String> = dx_audit::backend::vuln_locks("cargo")
        .iter()
        .map(|rel| (*rel).to_owned())
        .collect();
    locks.extend(set_locks(workspace, Ecosystem::Cargo, CARGO_LOCK)?);
    locks.retain(|rel| workspace.join(rel).is_file());
    Ok(locks)
}

/// Lock files named `file` of the dependency sets of one ecosystem.
fn set_locks(
    workspace: &Path,
    ecosystem: Ecosystem,
    file: &str,
) -> Result<BTreeSet<String>, PrepError> {
    let mut locks = BTreeSet::new();
    let registry =
        dx_adopt::dependency_sets::load(workspace).map_err(|error| PrepError::Usage {
            detail: error.to_string(),
        })?;
    for set in registry.iter().flat_map(|registry| &registry.sets) {
        if set.ecosystem == ecosystem {
            locks.extend(
                set.locks
                    .iter()
                    .filter(|rel| rel.rsplit('/').next() == Some(file))
                    .cloned(),
            );
        }
    }
    Ok(locks)
}
