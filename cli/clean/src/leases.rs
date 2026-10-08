use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_setup::GenerationId;

use super::records::GenerationKind;
use super::CleanError;

pub const LEASES_DIR_NAME: &str = "leases";

pub const LEASE_TIMEOUT: Duration = Duration::from_secs(10);

pub fn lease_path(dx_dir: &Path, kind: GenerationKind, hex: &str) -> PathBuf {
    dx_dir.join(LEASES_DIR_NAME).join(kind.dir_name()).join(hex)
}

fn open_lease_file(path: &Path) -> Result<File, CleanError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CleanError::LockFailed {
            path: path.to_path_buf(),
            reason: format!("cannot create lease dir: {e}"),
        })?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| CleanError::LockFailed {
            path: path.to_path_buf(),
            reason: format!("cannot open lease file: {e}"),
        })
}

pub struct GenerationLease {
    file: Option<File>,
    path: PathBuf,
}

impl GenerationLease {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for GenerationLease {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = file.unlock();
        }
    }
}

fn checked_lease_path(
    dx_dir: &Path,
    kind: GenerationKind,
    hex: &str,
) -> Result<PathBuf, CleanError> {
    if GenerationId::new(hex).is_err() {
        return Err(CleanError::LockFailed {
            path: dx_dir.to_path_buf(),
            reason: format!("{hex:?} is not a generation digest; refusing lease"),
        });
    }
    Ok(lease_path(dx_dir, kind, hex))
}

pub fn acquire_shared_lease(
    dx_dir: &Path,
    kind: GenerationKind,
    hex: &str,
    timeout: Duration,
) -> Result<GenerationLease, CleanError> {
    let path = checked_lease_path(dx_dir, kind, hex)?;
    let file = open_lease_file(&path)?;
    match dx_atomic_fs::lock_shared(&file, timeout) {
        Ok(()) => Ok(GenerationLease {
            file: Some(file),
            path,
        }),
        Err(std::fs::TryLockError::WouldBlock) => Err(CleanError::Busy { path }),
        Err(error) => Err(CleanError::LockFailed {
            path,
            reason: format!("cannot lock lease file: {error}"),
        }),
    }
}

pub fn try_exclusive_lease(
    dx_dir: &Path,
    kind: GenerationKind,
    hex: &str,
) -> Result<Option<GenerationLease>, CleanError> {
    let path = checked_lease_path(dx_dir, kind, hex)?;
    let file = open_lease_file(&path)?;
    match dx_atomic_fs::lock_exclusive(&file, Duration::ZERO) {
        Ok(()) => Ok(Some(GenerationLease {
            file: Some(file),
            path,
        })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(error) => Err(CleanError::LockFailed {
            path,
            reason: format!("cannot lock lease file: {error}"),
        }),
    }
}

pub fn remove_lease_file(dx_dir: &Path, kind: GenerationKind, hex: &str) {
    let path = lease_path(dx_dir, kind, hex);
    let _ = std::fs::remove_file(&path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;

    fn lease_dir(root: &Path) -> PathBuf {
        workspace_of(root).join(dx_env::DX_DIR_NAME)
    }

    #[test]
    fn shared_leases_coexist_and_block_exclusive() {
        let scratch = dx_test_scratch::scratch("dx-clean-test-lease-shared-");
        let root = scratch.path().to_path_buf();
        let dx_dir = lease_dir(&root);
        let hex = digest('1');
        let first = acquire_shared_lease(&dx_dir, GenerationKind::Environment, &hex, LEASE_TIMEOUT)
            .expect("first shared");
        assert_eq!(
            first.path(),
            lease_path(&dx_dir, GenerationKind::Environment, &hex)
        );
        let _second =
            acquire_shared_lease(&dx_dir, GenerationKind::Environment, &hex, LEASE_TIMEOUT)
                .expect("second shared");
        let blocked = try_exclusive_lease(&dx_dir, GenerationKind::Environment, &hex)
            .expect("exclusive attempt");
        assert!(blocked.is_none(), "shared holders block prune");
        drop(first);
        drop(_second);
        let held = try_exclusive_lease(&dx_dir, GenerationKind::Environment, &hex)
            .expect("exclusive after release");
        assert!(held.is_some(), "release frees the lease");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn exclusive_holder_blocks_shared_acquire() {
        let scratch = dx_test_scratch::scratch("dx-clean-test-lease-exclusive-");
        let root = scratch.path().to_path_buf();
        let dx_dir = lease_dir(&root);
        let hex = digest('2');
        let held = try_exclusive_lease(&dx_dir, GenerationKind::Generated, &hex)
            .expect("exclusive acquire")
            .expect("uncontended exclusive");
        let busy = acquire_shared_lease(&dx_dir, GenerationKind::Generated, &hex, Duration::ZERO);
        assert!(matches!(busy, Err(CleanError::Busy { .. })));
        drop(held);
        acquire_shared_lease(&dx_dir, GenerationKind::Generated, &hex, LEASE_TIMEOUT)
            .expect("shared after release");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn non_digest_hex_is_refused_without_creating_state() {
        let scratch = dx_test_scratch::scratch("dx-clean-test-lease-refuse-");
        let root = scratch.path().to_path_buf();
        let dx_dir = lease_dir(&root);
        assert!(matches!(
            acquire_shared_lease(
                &dx_dir,
                GenerationKind::Environment,
                "latest",
                LEASE_TIMEOUT
            ),
            Err(CleanError::LockFailed { .. })
        ));
        assert!(matches!(
            try_exclusive_lease(&dx_dir, GenerationKind::Environment, "latest"),
            Err(CleanError::LockFailed { .. })
        ));
        assert!(
            !dx_dir.join(LEASES_DIR_NAME).exists(),
            "refused leases create no state"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
