use std::collections::BTreeSet;
use std::path::Path;

use dx_audit::advisory_prep::{yanked_snapshot, PrepError, PreparedSnapshot};
use dx_update::discover::cargo::{index_path, yanked_versions, DEFAULT_BASE};

use crate::{fetch_many, PrepareOptions};

pub const CARGO_YANKED: &str = "cargo-yanked";

/// Where the cargo index is read from: the options value or the crates.io sparse index.
pub fn index_base(options: &PrepareOptions) -> &str {
    options.cargo_index.as_deref().unwrap_or(DEFAULT_BASE)
}

/// Builds the yanked-version snapshot for the cargo locks of the workspace.
pub fn prepare_cargo_yanked(options: &PrepareOptions) -> Result<PreparedSnapshot, PrepError> {
    let base = index_base(options).trim_end_matches('/').to_owned();
    let locked = locked_registry_crates(&options.workspace)?;
    let scratch = tempfile::Builder::new()
        .prefix("dx-yanked-")
        .tempdir()
        .map_err(|error| PrepError::ArchiveRead {
            family: CARGO_YANKED.to_owned(),
            path: std::env::temp_dir().display().to_string(),
            detail: error.to_string(),
        })?;
    let downloads: Vec<(String, std::path::PathBuf)> = locked
        .keys()
        .enumerate()
        .map(|(index, name)| {
            (
                format!("{base}/{}", index_path(name)),
                scratch.path().join(format!("{index}.json")),
            )
        })
        .collect();
    fetch_many(&downloads, options.limits)?;
    let mut yanked = Vec::new();
    for ((name, versions), (url, file)) in locked.iter().zip(&downloads) {
        let body = std::fs::read_to_string(file).map_err(|error| PrepError::ArchiveRead {
            family: CARGO_YANKED.to_owned(),
            path: url.clone(),
            detail: error.to_string(),
        })?;
        let listed = yanked_versions(&body).map_err(|detail| PrepError::Usage {
            detail: format!("{url}: {detail}"),
        })?;
        for version in versions.iter().filter(|version| listed.contains(version)) {
            yanked.push((name.clone(), version.clone()));
        }
    }
    yanked_snapshot(CARGO_YANKED, &base, &options.retrieved_at, "cargo", &yanked)
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
    let registry =
        dx_adopt::dependency_sets::load(workspace).map_err(|error| PrepError::Usage {
            detail: error.to_string(),
        })?;
    for set in registry.iter().flat_map(|registry| &registry.sets) {
        if set.ecosystem == dx_adopt::dependency_sets::Ecosystem::Cargo {
            locks.extend(
                set.locks
                    .iter()
                    .filter(|rel| {
                        rel.rsplit('/').next() == Some(dx_adopt::dependency_sets::CARGO_LOCK)
                    })
                    .cloned(),
            );
        }
    }
    locks.retain(|rel| workspace.join(rel).is_file());
    Ok(locks)
}
