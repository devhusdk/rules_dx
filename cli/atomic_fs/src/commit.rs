//! Shared commit lock, managed-root open, and staged pointer transitions.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::lock_exclusive;
use crate::managed::{classify, create_pointer, remove_managed, EntryKind, PointerKind};

/// Name of the managed root directory inside a workspace.
pub const DX_DIR_NAME: &str = ".dx";

/// Name of the commit lock file inside the managed root.
pub const LOCK_FILE_NAME: &str = ".commit.lock";

/// Deadline for acquiring the commit lock.
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Why the commit lock could not be held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// Another holder kept the lock past the deadline.
    Busy { path: PathBuf },
    /// The lock file could not be opened or locked.
    LockFailed { path: PathBuf, reason: String },
}

impl fmt::Display for LockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::Busy { path } => write!(
                formatter,
                "another refresh holds {}; giving up after the commit-lock deadline",
                path.display()
            ),
            LockError::LockFailed { path, reason } => {
                write!(formatter, "cannot lock {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for LockError {}

/// Why the managed root could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootError {
    /// The workspace root is missing or is not a directory.
    WorkspaceRoot { path: PathBuf },
    /// The managed root could not be created.
    Create { path: PathBuf, reason: String },
}

impl fmt::Display for RootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RootError::WorkspaceRoot { path } => {
                write!(formatter, "workspace root {} is not a directory", path.display())
            }
            RootError::Create { path, reason } => {
                write!(formatter, "cannot create {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for RootError {}

/// Why a staged pointer transition failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    /// The staged entry could not be inspected.
    Inspect { path: PathBuf, reason: String },
    /// The staged entry is a directory that is not ours.
    Foreign { path: PathBuf },
    /// The staged entry could not be removed.
    Clear { path: PathBuf, reason: String },
    /// The staged pointer could not be created.
    Stage { path: PathBuf, reason: String },
    /// The staged pointer could not replace the current pointer.
    Publish { path: PathBuf, reason: String },
}

impl fmt::Display for StageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageError::Inspect { path, reason } => {
                write!(formatter, "cannot inspect stale {}: {reason}", path.display())
            }
            StageError::Foreign { path } => write!(
                formatter,
                "stale {} is a directory; refusing to adopt foreign state",
                path.display()
            ),
            StageError::Clear { path, reason } => {
                write!(formatter, "cannot clear stale {}: {reason}", path.display())
            }
            StageError::Stage { path, reason } => {
                write!(formatter, "cannot stage {}: {reason}", path.display())
            }
            StageError::Publish { path, reason } => write!(
                formatter,
                "cannot publish {}: {reason}; the prior pointer is preserved",
                path.display()
            ),
        }
    }
}

impl std::error::Error for StageError {}

/// Opens the managed root directory for commit work.
pub fn managed_root(workspace_root: &Path) -> Result<PathBuf, RootError> {
    if !workspace_root.is_dir() {
        return Err(RootError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let root = workspace_root.join(DX_DIR_NAME);
    fs::create_dir_all(&root).map_err(|error| RootError::Create {
        path: root.clone(),
        reason: error.to_string(),
    })?;
    Ok(root)
}

/// Holds the commit lock on the managed root until the returned guard drops.
pub fn acquire_lock(dx_dir: &Path, timeout: Duration) -> Result<File, LockError> {
    let path = dx_dir.join(LOCK_FILE_NAME);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| LockError::LockFailed {
            path: path.clone(),
            reason: format!("cannot open commit lock: {error}"),
        })?;
    match lock_exclusive(&file, timeout) {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(LockError::Busy { path }),
        Err(error) => Err(LockError::LockFailed {
            path,
            reason: format!("cannot lock commit lock: {error}"),
        }),
    }
}

/// Removes a stale staged entry without following it into its target.
pub fn clear_staged_pointer(stage: &Path) -> Result<(), StageError> {
    match classify(stage) {
        Err(error) => Err(StageError::Inspect {
            path: stage.to_path_buf(),
            reason: error.to_string(),
        }),
        Ok(EntryKind::Missing) => Ok(()),
        Ok(EntryKind::Directory) => Err(StageError::Foreign {
            path: stage.to_path_buf(),
        }),
        Ok(EntryKind::File | EntryKind::Link) => {
            remove_managed(stage).map_err(|error| StageError::Clear {
                path: stage.to_path_buf(),
                reason: error.to_string(),
            })
        }
    }
}

/// Points a staged entry at a target generation.
pub fn stage_pointer(target: &Path, stage: &Path, kind: PointerKind) -> Result<(), StageError> {
    create_pointer(target, stage, kind).map_err(|error| StageError::Stage {
        path: stage.to_path_buf(),
        reason: error.to_string(),
    })
}

/// Replaces the current pointer with the staged one after a successful rename.
pub fn publish_staged(stage: &Path, current: &Path) -> Result<(), StageError> {
    fs::rename(stage, current).map_err(|error| StageError::Publish {
        path: current.to_path_buf(),
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(windows)]
    fn directory_pointer(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(windows))]
    fn directory_pointer(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn file_pointer(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[cfg(not(windows))]
    fn file_pointer(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    fn scratch_workspace(prefix: &str) -> (dx_test_scratch::TempDir, PathBuf) {
        let scratch = dx_test_scratch::scratch(prefix);
        let workspace = scratch.path().join("ws");
        fs::create_dir_all(&workspace).expect("create workspace");
        (scratch, workspace)
    }

    fn setups_dir(workspace: &Path) -> PathBuf {
        workspace.join(DX_DIR_NAME).join("setups")
    }

    #[test]
    fn managed_root_creates_the_root_and_refuses_missing_workspaces() {
        let (scratch, workspace) = scratch_workspace("dx-commit-root-");
        let missing = workspace.join("no-such-dir");
        assert_eq!(
            managed_root(&missing).expect_err("missing workspace"),
            RootError::WorkspaceRoot {
                path: missing.clone()
            }
        );
        assert_eq!(
            managed_root(&missing)
                .expect_err("missing workspace")
                .to_string(),
            format!("workspace root {} is not a directory", missing.display())
        );
        let root = managed_root(&workspace).expect("create root");
        assert_eq!(root, workspace.join(DX_DIR_NAME));
        assert!(root.is_dir());
        assert_eq!(managed_root(&workspace).expect("reuse root"), root);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn managed_root_reports_an_uncreatable_root() {
        let (scratch, workspace) = scratch_workspace("dx-commit-root-create-");
        fs::write(&workspace.join(DX_DIR_NAME), b"not a directory\n").expect("file root");
        let error = managed_root(&workspace).expect_err("root is a file");
        assert!(matches!(error, RootError::Create { .. }));
        assert!(
            error.to_string().starts_with("cannot create "),
            "{error}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn lock_errors_display_the_holder_and_reason() {
        let busy = LockError::Busy {
            path: PathBuf::from("/ws/.dx/.commit.lock"),
        };
        assert_eq!(
            busy.to_string(),
            "another refresh holds /ws/.dx/.commit.lock; giving up after the commit-lock deadline"
        );
        let failed = LockError::LockFailed {
            path: PathBuf::from("/ws/.dx/.commit.lock"),
            reason: "cannot open commit lock: denied".to_string(),
        };
        assert_eq!(
            failed.to_string(),
            "cannot lock /ws/.dx/.commit.lock: cannot open commit lock: denied"
        );
    }

    #[test]
    fn acquire_lock_serializes_and_reports_contention() {
        let (scratch, workspace) = scratch_workspace("dx-commit-lock-busy-");
        let root = managed_root(&workspace).expect("root");
        let held = acquire_lock(&root, Duration::from_secs(10)).expect("hold lock");
        let error = acquire_lock(&root, Duration::ZERO).expect_err("lock is held");
        assert_eq!(
            error,
            LockError::Busy {
                path: root.join(LOCK_FILE_NAME)
            }
        );
        drop(held);
        let next = acquire_lock(&root, Duration::from_secs(10)).expect("reacquire");
        drop(next);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn acquire_lock_reports_an_unopenable_lock_file() {
        let (scratch, workspace) = scratch_workspace("dx-commit-lock-open-");
        let root = managed_root(&workspace).expect("root");
        fs::create_dir(root.join(LOCK_FILE_NAME)).expect("lock is a directory");
        let error = acquire_lock(&root, Duration::from_secs(1)).expect_err("cannot open");
        assert!(matches!(error, LockError::LockFailed { .. }));
        assert!(
            error.to_string().contains("cannot open commit lock"),
            "{error}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn clear_staged_pointer_reclaims_links_and_files() {
        let (scratch, workspace) = scratch_workspace("dx-commit-clear-");
        let stage = workspace.join("current.next");
        clear_staged_pointer(&stage).expect("missing stage needs no clearing");

        directory_pointer(Path::new("missing-target"), &stage).expect("dangling link");
        clear_staged_pointer(&stage).expect("clear dangling link");
        assert!(stage.symlink_metadata().is_err(), "dangling link removed");

        let target = workspace.join("target.txt");
        fs::write(&target, b"keep\n").expect("target");
        file_pointer(&target, &stage).expect("file link");
        clear_staged_pointer(&stage).expect("clear file link");
        assert!(stage.symlink_metadata().is_err(), "file link removed");
        assert_eq!(fs::read(&target).expect("target bytes"), b"keep\n");

        fs::write(&stage, b"foreign\n").expect("foreign staged file");
        clear_staged_pointer(&stage).expect("clear foreign staged file");
        assert!(stage.symlink_metadata().is_err(), "foreign file removed");

        fs::create_dir(&stage).expect("foreign staged directory");
        assert_eq!(
            clear_staged_pointer(&stage).expect_err("a directory is refused"),
            StageError::Foreign { path: stage.clone() }
        );
        assert!(stage.is_dir(), "the refused directory survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn clear_staged_pointer_reports_an_uninspectable_stage() {
        let (scratch, workspace) = scratch_workspace("dx-commit-clear-inspect-");
        let parent = workspace.join("plain.txt");
        fs::write(&parent, b"plain\n").expect("file parent");
        let error = clear_staged_pointer(&parent.join("current.next")).expect_err("cannot stat");
        assert!(matches!(error, StageError::Inspect { .. }));
        assert!(
            error.to_string().starts_with("cannot inspect stale "),
            "{error}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn stage_pointer_reports_an_occupied_stage() {
        let (scratch, workspace) = scratch_workspace("dx-commit-stage-occupied-");
        let record = workspace.join("record");
        fs::create_dir(&record).expect("record");
        let stage = workspace.join("current.next");
        fs::create_dir(&stage).expect("occupied stage");
        let error =
            stage_pointer(&record, &stage, PointerKind::Directory).expect_err("stage is taken");
        assert!(matches!(error, StageError::Stage { .. }));
        assert!(error.to_string().starts_with("cannot stage "), "{error}");
        assert!(record.is_dir(), "the record survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn publish_staged_swaps_the_pointer_and_consumes_the_stage() {
        let (scratch, workspace) = scratch_workspace("dx-commit-publish-");
        let setups = setups_dir(&workspace);
        fs::create_dir_all(&setups).expect("setups dir");
        let current = setups.join("current");
        let stage = setups.join("current.next");
        let prior = setups.join("prior-record");
        fs::create_dir(&prior).expect("prior record");
        stage_pointer(&prior, &current, PointerKind::Directory).expect("prior pointer");

        let record = setups.join("record");
        fs::create_dir(&record).expect("record");
        stage_pointer(&record, &stage, PointerKind::Directory).expect("stage record");
        assert_eq!(fs::read_link(&stage).expect("staged"), record);
        publish_staged(&stage, &current).expect("publish");
        assert_eq!(fs::read_link(&current).expect("current"), record);
        assert!(stage.symlink_metadata().is_err(), "stage consumed");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn publish_staged_failure_keeps_the_prior_pointer() {
        let (scratch, workspace) = scratch_workspace("dx-commit-publish-fail-");
        let setups = setups_dir(&workspace);
        fs::create_dir_all(&setups).expect("setups dir");
        let current = setups.join("current");
        let stage = setups.join("current.next");
        let prior = setups.join("prior-record");
        fs::create_dir(&prior).expect("prior record");
        stage_pointer(&prior, &current, PointerKind::Directory).expect("prior pointer");

        let error = publish_staged(&stage, &current).expect_err("nothing staged");
        assert!(matches!(error, StageError::Publish { .. }));
        assert!(
            error
                .to_string()
                .ends_with("; the prior pointer is preserved"),
            "{error}"
        );
        assert_eq!(
            fs::read_link(&current).expect("prior selection"),
            prior,
            "the failed publish leaves the prior selection in place"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn publish_staged_failure_in_a_read_only_dir_keeps_both_entries() {
        use std::os::unix::fs::PermissionsExt as _;

        let (scratch, workspace) = scratch_workspace("dx-commit-publish-readonly-");
        let setups = setups_dir(&workspace);
        fs::create_dir_all(&setups).expect("setups dir");
        let current = setups.join("current");
        let stage = setups.join("current.next");
        let prior = setups.join("prior-record");
        fs::create_dir(&prior).expect("prior record");
        stage_pointer(&prior, &current, PointerKind::Directory).expect("prior pointer");
        let record = setups.join("record");
        fs::create_dir(&record).expect("record");
        stage_pointer(&record, &stage, PointerKind::Directory).expect("stage record");

        fs::set_permissions(&setups, fs::Permissions::from_mode(0o555)).expect("read-only dir");
        let error = publish_staged(&stage, &current).expect_err("rename is denied");
        fs::set_permissions(&setups, fs::Permissions::from_mode(0o755)).expect("writable again");
        assert!(matches!(error, StageError::Publish { .. }));
        assert_eq!(fs::read_link(&current).expect("prior selection"), prior);
        assert_eq!(fs::read_link(&stage).expect("staged pointer"), record);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(windows)]
    fn staged_directory_link_cleanup_keeps_the_target_tree() {
        let (scratch, workspace) = scratch_workspace("dx-commit-windows-stage-");
        let record = workspace.join("record");
        fs::create_dir_all(record.join("nested")).expect("record tree");
        fs::write(record.join("nested").join("entry.txt"), b"keep\n").expect("record entry");
        let stage = workspace.join("current.next");
        stage_pointer(&record, &stage, PointerKind::Directory).expect("stage record");
        clear_staged_pointer(&stage).expect("clear staged directory link");
        assert!(stage.symlink_metadata().is_err(), "staged link removed");
        assert_eq!(
            fs::read(record.join("nested").join("entry.txt")).expect("record entry"),
            b"keep\n",
            "the record tree survives link cleanup"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn commit_errors_display() {
        let messages = [
            LockError::Busy {
                path: PathBuf::from("/ws/.dx/.commit.lock"),
            }
            .to_string(),
            LockError::LockFailed {
                path: PathBuf::from("/ws/.dx/.commit.lock"),
                reason: "r".to_string(),
            }
            .to_string(),
            RootError::WorkspaceRoot {
                path: PathBuf::from("/ws"),
            }
            .to_string(),
            RootError::Create {
                path: PathBuf::from("/ws/.dx"),
                reason: "r".to_string(),
            }
            .to_string(),
            StageError::Inspect {
                path: PathBuf::from("/ws/current.next"),
                reason: "r".to_string(),
            }
            .to_string(),
            StageError::Foreign {
                path: PathBuf::from("/ws/current.next"),
            }
            .to_string(),
            StageError::Clear {
                path: PathBuf::from("/ws/current.next"),
                reason: "r".to_string(),
            }
            .to_string(),
            StageError::Stage {
                path: PathBuf::from("/ws/current.next"),
                reason: "r".to_string(),
            }
            .to_string(),
            StageError::Publish {
                path: PathBuf::from("/ws/current"),
                reason: "r".to_string(),
            }
            .to_string(),
        ];
        for message in &messages {
            assert!(!message.is_empty());
        }
    }
}
