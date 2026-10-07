//! One managed-root commit lock plus staged pointer publication.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::managed::{create_pointer, remove_managed, PointerKind};

/// Name of the managed root every generation domain coordinates under.
pub const DX_DIR_NAME: &str = ".dx";

/// Lock file inside a managed root that serializes generation commits.
pub const COMMIT_LOCK_FILE_NAME: &str = ".commit.lock";

/// Deadline every domain waits for the commit lock before reporting busy.
pub const DEFAULT_COMMIT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Lock acquisition failure, without any domain policy attached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    #[error("another commit holds {path:?}; giving up after the lock deadline", path = path.display())]
    Busy { path: PathBuf },
    #[error("cannot lock {path:?}: {reason}", path = path.display())]
    LockFailed { path: PathBuf, reason: String },
}

/// Staged commit failure, without any domain policy attached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    #[error("refusing foreign state at {path:?}: {reason}", path = path.display())]
    Foreign { path: PathBuf, reason: String },
    #[error("commit failed: {reason}")]
    Install { reason: String },
}

/// A managed root directory plus the lock file it owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRoot {
    dir: PathBuf,
}

impl ManagedRoot {
    /// Names a managed root; use [`ManagedRoot::ensure_dir`] before locking.
    pub fn new(dir: PathBuf) -> Self {
        ManagedRoot { dir }
    }

    /// The root directory itself.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The lock file serializing this root.
    pub fn lock_path(&self) -> PathBuf {
        self.dir.join(COMMIT_LOCK_FILE_NAME)
    }

    /// Creates the root when missing; a blocking file fails the commit.
    pub fn ensure_dir(&self) -> Result<(), CommitError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| CommitError::Install {
            reason: format!("cannot create {}: {e}", self.dir.display()),
        })
    }
}

/// An acquired commit lock; dropping releases it.
#[derive(Debug)]
pub struct CommitLock {
    file: File,
}

impl CommitLock {
    /// Hands the locked file to a caller that keeps the raw handle.
    pub fn into_file(self) -> File {
        self.file
    }
}

/// Opens the root lock file and waits up to `timeout` for exclusivity.
pub fn acquire_commit_lock(root: &ManagedRoot, timeout: Duration) -> Result<CommitLock, LockError> {
    let path = root.lock_path();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| LockError::LockFailed {
            path: path.clone(),
            reason: format!("cannot open commit lock: {e}"),
        })?;
    match super::lock_exclusive(&file, timeout) {
        Ok(()) => Ok(CommitLock { file }),
        Err(std::fs::TryLockError::WouldBlock) => Err(LockError::Busy { path: path.clone() }),
        Err(e) => Err(LockError::LockFailed {
            path: path.clone(),
            reason: format!("cannot lock commit lock: {e}"),
        }),
    }
}

/// One staged pointer publication: `stage` names `target`, then replaces `current`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerSwap {
    pub current: PathBuf,
    pub stage: PathBuf,
    pub target: PathBuf,
    pub kind: PointerKind,
}

/// Removes a leftover staged entry without following it into its target.
pub fn clear_stale_stage(stage: &Path) -> Result<(), CommitError> {
    match std::fs::symlink_metadata(stage) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CommitError::Install {
            reason: format!("cannot inspect stale {}: {e}", stage.display()),
        }),
        Ok(meta) => {
            if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
                return Err(CommitError::Foreign {
                    path: stage.to_path_buf(),
                    reason: format!(
                        "stale {} is a directory; refusing to adopt foreign state",
                        stage.display()
                    ),
                });
            }
            remove_managed(stage).map_err(|e| CommitError::Install {
                reason: format!("cannot clear stale {}: {e}", stage.display()),
            })
        }
    }
}

/// Stages `target` and atomically publishes it; a failed rename keeps the prior pointer.
pub fn publish_pointer_swap(swap: &PointerSwap) -> Result<(), CommitError> {
    clear_stale_stage(&swap.stage)?;
    create_pointer(&swap.target, &swap.stage, swap.kind).map_err(|e| CommitError::Install {
        reason: format!("cannot stage {}: {e}", swap.stage.display()),
    })?;
    std::fs::rename(&swap.stage, &swap.current).map_err(|e| CommitError::Install {
        reason: format!(
            "cannot publish {}: {e}; the prior pointer is preserved",
            swap.current.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_at(dir: &Path) -> ManagedRoot {
        ManagedRoot::new(dir.to_path_buf())
    }

    #[test]
    fn second_acquire_reports_busy_with_the_lock_path() {
        let scratch = dx_test_scratch::scratch("dx-commit-lock-");
        let root = root_at(&scratch.path().join(DX_DIR_NAME));
        root.ensure_dir().expect("root");
        assert_eq!(
            root.lock_path(),
            scratch.path().join(DX_DIR_NAME).join(COMMIT_LOCK_FILE_NAME)
        );
        let _held = acquire_commit_lock(&root, Duration::from_secs(10)).expect("hold commit lock");
        let busy = acquire_commit_lock(&root, Duration::ZERO).expect_err("locked root is busy");
        assert_eq!(
            busy,
            LockError::Busy {
                path: root.lock_path()
            }
        );
        assert!(!LockError::Busy {
            path: root.lock_path()
        }
        .to_string()
        .is_empty());
        drop(_held);
        acquire_commit_lock(&root, Duration::ZERO).expect("released lock acquires");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn lock_open_failure_reports_the_path_and_keeps_foreign_state() {
        let scratch = dx_test_scratch::scratch("dx-commit-lock-open-");
        let root = root_at(&scratch.path().join(DX_DIR_NAME));
        root.ensure_dir().expect("root");
        std::fs::create_dir_all(root.lock_path()).expect("lock is a directory");
        let error = acquire_commit_lock(&root, Duration::from_secs(1)).expect_err("open fails");
        match &error {
            LockError::LockFailed { path, reason } => {
                assert_eq!(path, &root.lock_path());
                assert!(!reason.is_empty());
            }
            LockError::Busy { .. } => panic!("a directory lock file is not busy: {error:?}"),
        }
        assert!(root.lock_path().is_dir(), "the foreign directory survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn root_ensure_refuses_a_blocking_file() {
        let scratch = dx_test_scratch::scratch("dx-commit-root-");
        let blocking = scratch.path().join(DX_DIR_NAME);
        std::fs::write(&blocking, "foreign").expect("blocking file");
        let error = root_at(&blocking)
            .ensure_dir()
            .expect_err("file blocks root");
        assert!(matches!(error, CommitError::Install { .. }));
        assert_eq!(
            std::fs::read(&blocking).expect("foreign preserved"),
            b"foreign"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn handed_off_handles_keep_the_lock() {
        let scratch = dx_test_scratch::scratch("dx-commit-handoff-");
        let root = root_at(&scratch.path().join(DX_DIR_NAME));
        root.ensure_dir().expect("root");
        let lock = acquire_commit_lock(&root, Duration::from_secs(10)).expect("acquire");
        let file = lock.into_file();
        super::super::lock_exclusive(&file, Duration::ZERO).expect("handle stays locked");
        scratch.close().expect("cleanup");
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(windows))]
    fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[cfg(not(windows))]
    fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    fn links_allowed(probe: io::Result<()>) -> bool {
        if probe.is_ok() {
            return true;
        }
        eprintln!("host refuses links; link cases are covered on hosts that allow them");
        false
    }

    #[test]
    fn stale_stage_clear_matches_the_entry_kind() {
        let scratch = dx_test_scratch::scratch("dx-commit-stage-kinds-");
        let stage = scratch.path().join("current.next");
        assert!(
            clear_stale_stage(&stage).is_ok(),
            "a missing staged entry needs no clearing"
        );

        let probe = scratch.path().join("probe");
        if links_allowed(symlink_dir(Path::new("missing-target"), &probe)) {
            remove_managed(&probe).expect("remove probe");
            symlink_dir(Path::new("missing-target"), &stage).expect("dangling directory link");
            clear_stale_stage(&stage).expect("clear dangling directory link");
            assert!(stage.symlink_metadata().is_err(), "directory link removed");

            let file = scratch.path().join("foreign.txt");
            std::fs::write(&file, "foreign").expect("foreign file");
            symlink_file(&file, &stage).expect("file link");
            clear_stale_stage(&stage).expect("clear file link");
            assert!(stage.symlink_metadata().is_err(), "file link removed");
            assert!(file.is_file(), "the linked file survives");
        }

        std::fs::write(&stage, "foreign").expect("foreign staged file");
        clear_stale_stage(&stage).expect("clear foreign staged file");
        assert!(stage.symlink_metadata().is_err(), "file removed");

        std::fs::create_dir(&stage).expect("foreign staged dir");
        let error = clear_stale_stage(&stage).expect_err("a staged directory stays refused");
        assert!(matches!(error, CommitError::Foreign { .. }));
        assert!(stage.is_dir(), "the refused directory survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn stale_stage_inside_a_file_reports_without_clearing() {
        let scratch = dx_test_scratch::scratch("dx-commit-stage-parent-");
        let file = scratch.path().join("foreign");
        std::fs::write(&file, "foreign").expect("foreign file");
        let error = clear_stale_stage(&file.join("child")).expect_err("uninspectable stage fails");
        assert!(matches!(error, CommitError::Install { .. }));
        assert_eq!(std::fs::read(&file).expect("foreign preserved"), b"foreign");
        scratch.close().expect("cleanup");
    }

    fn swap_in(dir: &Path, target: &str) -> PointerSwap {
        PointerSwap {
            current: dir.join("current"),
            stage: dir.join("current.next"),
            target: PathBuf::from(target),
            kind: PointerKind::Directory,
        }
    }

    #[test]
    fn publish_fresh_swap_installs_the_pointer() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-fresh-");
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        let swap = swap_in(&dir, "record-a");
        publish_pointer_swap(&swap).expect("fresh publish");
        assert_eq!(
            std::fs::read_link(&swap.current).expect("pointer target"),
            PathBuf::from("record-a")
        );
        assert!(
            swap.stage.symlink_metadata().is_err(),
            "the staged entry is consumed by the rename"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn republish_same_swap_is_idempotent() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-idempotent-");
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        let swap = swap_in(&dir, "record-a");
        publish_pointer_swap(&swap).expect("first publish");
        publish_pointer_swap(&swap).expect("republish");
        assert_eq!(
            std::fs::read_link(&swap.current).expect("pointer target"),
            PathBuf::from("record-a")
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn publish_replaces_the_prior_pointer() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-replace-");
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        publish_pointer_swap(&swap_in(&dir, "record-a")).expect("first publish");
        publish_pointer_swap(&swap_in(&dir, "record-b")).expect("replacement publish");
        assert_eq!(
            std::fs::read_link(dir.join("current")).expect("pointer target"),
            PathBuf::from("record-b")
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn publish_clears_a_stale_file_stage_first() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-stale-");
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        std::fs::write(dir.join("current.next"), "stale").expect("stale stage file");
        publish_pointer_swap(&swap_in(&dir, "record-a")).expect("publish over stale stage");
        assert_eq!(
            std::fs::read_link(dir.join("current")).expect("pointer target"),
            PathBuf::from("record-a")
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn publish_failure_keeps_the_prior_pointer() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-failure-");
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        publish_pointer_swap(&swap_in(&dir, "record-a")).expect("first publish");
        let current = dir.join("current");
        dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
        std::fs::create_dir(&current).expect("blocking directory");
        let marker = current.join("kept.txt");
        std::fs::write(&marker, "prior").expect("prior marker");
        let error = publish_pointer_swap(&swap_in(&dir, "record-b")).expect_err("rename must fail");
        assert!(matches!(error, CommitError::Install { .. }));
        assert_eq!(
            std::fs::read(&marker).expect("prior state survives"),
            b"prior"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn commit_errors_display() {
        let errors = [
            CommitError::Foreign {
                path: PathBuf::from("/ws/.dx/current.next"),
                reason: "r".to_string(),
            },
            CommitError::Install {
                reason: "r".to_string(),
            },
        ];
        for error in &errors {
            assert!(!format!("{error}").is_empty());
        }
        let busy = LockError::Busy {
            path: PathBuf::from("/ws/.dx/.commit.lock"),
        };
        assert!(!format!("{busy}").is_empty());
    }
}
