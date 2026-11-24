use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{SetupPair, read_current_pair};
use dx_atomic_fs::commit::{ManagedRoot, acquire_commit_lock};
use dx_atomic_fs::lease::{self, GenerationUse, SharedLease};

use crate::prune::PruneError;
use crate::prune::apply::map_lock_error;

#[derive(Debug, thiserror::Error)]
pub enum HoldError {
    #[error(transparent)]
    Prune(#[from] PruneError),
    #[error("generation .dx/local/{kind}/{hex} is being removed; retry after cleanup finishes")]
    Removing { kind: &'static str, hex: String },
    #[error("cannot lease .dx/local/{kind}/{hex}: {reason}")]
    Lease {
        kind: &'static str,
        hex: String,
        reason: String,
    },
    #[error("selected generation {path:?} is not usable: {reason}", path = path.display())]
    Generation { path: PathBuf, reason: String },
}

#[derive(Debug)]
pub struct HeldSelection {
    pair: SetupPair,
    leases: Vec<SharedLease>,
}

impl HeldSelection {
    #[must_use]
    pub fn pair(&self) -> &SetupPair {
        &self.pair
    }

    pub fn into_files(self) -> Vec<File> {
        self.leases
            .into_iter()
            .map(SharedLease::into_file)
            .collect()
    }
}

pub fn hold_current(
    workspace_root: &Path,
    timeout: Duration,
) -> Result<Option<HeldSelection>, HoldError> {
    if !workspace_root.is_dir() {
        return Err(PruneError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        }
        .into());
    }
    let dx_dir = dx_path::state::local_dir(workspace_root);
    match fs::metadata(&dx_dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PruneError::CurrentInvalid {
                reason: format!("cannot inspect {}: {e}", dx_dir.display()),
            }
            .into());
        }
        Ok(meta) if !meta.is_dir() => {
            return Err(PruneError::CurrentInvalid {
                reason: format!("{} is not a directory", dx_dir.display()),
            }
            .into());
        }
        Ok(_) => {}
    }
    let _lock =
        acquire_commit_lock(&ManagedRoot::new(dx_dir.clone()), timeout).map_err(map_lock_error)?;
    let pair = read_current_pair(workspace_root).map_err(|e| PruneError::CurrentInvalid {
        reason: e.to_string(),
    })?;
    let Some(pair) = pair else {
        return Ok(None);
    };
    let mut leases = Vec::with_capacity(2);
    for (kind, hex) in [
        (GenerationUse::Environment, pair.environment.as_str()),
        (GenerationUse::Generated, pair.generated.as_str()),
    ] {
        leases.push(lease_side(&dx_dir, kind, hex)?);
        check_generation_dir(&dx_dir.join(kind.dir_name()).join(hex))?;
    }
    Ok(Some(HeldSelection { pair, leases }))
}

fn lease_side(dx_dir: &Path, kind: GenerationUse, hex: &str) -> Result<SharedLease, HoldError> {
    lease::acquire_shared(dx_dir, kind, hex, Duration::ZERO).map_err(|e| {
        if e.kind() == io::ErrorKind::WouldBlock {
            HoldError::Removing {
                kind: kind.dir_name(),
                hex: hex.to_owned(),
            }
        } else {
            HoldError::Lease {
                kind: kind.dir_name(),
                hex: hex.to_owned(),
                reason: e.to_string(),
            }
        }
    })
}

fn check_generation_dir(path: &Path) -> Result<(), HoldError> {
    let reason = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => return Ok(()),
        Ok(meta) if meta.file_type().is_symlink() => "is a link, not a directory".to_owned(),
        Ok(_) => "is not a directory".to_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => "is missing".to_owned(),
        Err(e) => format!("cannot inspect: {e}"),
    };
    Err(HoldError::Generation {
        path: path.to_path_buf(),
        reason,
    })
}

#[cfg(test)]
#[path = "hold_tests.rs"]
mod hold_tests;
