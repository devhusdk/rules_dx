//! Reader side of the generation-lease protocol.
//!
//! [`hold_current`] takes shared leases on both sides of the current
//! selection under the managed-root commit lock, the same lock cleanup holds
//! while it prunes (see [`crate::apply`]). A generation it returns therefore
//! exists in full and cannot be pruned until every holder of the leases is
//! gone. Pass [`HeldSelection::into_files`] to a launcher (for example
//! `dx_process` `StreamSpec::held`) to keep the leases for a child tree.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_atomic_fs::lease::{self, GenerationUse, SharedLease};
use dx_env::{acquire_lock, DX_DIR_NAME};
use dx_setup::{read_current_pair, SetupPair};

use super::apply::map_lock_error;
use super::CleanError;

/// Why the current selection could not be held.
#[derive(Debug, thiserror::Error)]
pub enum HoldError {
    #[error(transparent)]
    Clean(#[from] CleanError),
    #[error("generation .dx/{kind}/{hex} is being removed; retry after cleanup finishes")]
    Removing { kind: &'static str, hex: String },
    #[error("cannot lease .dx/{kind}/{hex}: {reason}")]
    Lease {
        kind: &'static str,
        hex: String,
        reason: String,
    },
    #[error("selected generation {path:?} is not usable: {reason}", path = path.display())]
    Generation { path: PathBuf, reason: String },
}

/// Shared leases on both generations of one selection; dropping releases them.
#[derive(Debug)]
pub struct HeldSelection {
    pair: SetupPair,
    leases: Vec<SharedLease>,
}

impl HeldSelection {
    /// The selection the leases protect.
    pub fn pair(&self) -> &SetupPair {
        &self.pair
    }

    /// The locked lease files, for a launcher that keeps them for a child tree.
    pub fn into_files(self) -> Vec<File> {
        self.leases
            .into_iter()
            .map(SharedLease::into_file)
            .collect()
    }

    /// The `DX_MANAGED_LEASES` value naming the held generations.
    ///
    /// This is information for the child only: an environment string is not
    /// a lock, so the leases themselves must travel as open files.
    pub fn lease_env(&self) -> String {
        lease::inherit_value(&[
            (GenerationUse::Environment, self.pair.environment.as_str()),
            (GenerationUse::Generated, self.pair.generated.as_str()),
        ])
    }
}

/// Holds shared leases on the current selection; `None` means nothing is selected.
///
/// Waits up to `timeout` for the commit lock. Lease contention under the
/// lock means cleanup holds the generation exclusively, which fails closed
/// with [`HoldError::Removing`] instead of waiting while holding the lock.
/// Never creates or repairs a generation directory.
pub fn hold_current(
    workspace_root: &Path,
    timeout: Duration,
) -> Result<Option<HeldSelection>, HoldError> {
    if !workspace_root.is_dir() {
        return Err(CleanError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        }
        .into());
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    if !dx_dir.is_dir() {
        return Ok(None);
    }
    let _lock = acquire_lock(&dx_dir, timeout).map_err(map_lock_error)?;
    let pair = read_current_pair(workspace_root).map_err(|e| CleanError::CurrentInvalid {
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
