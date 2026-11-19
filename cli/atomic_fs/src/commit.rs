//! One managed-root commit lock plus staged pointer publication.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::managed::{EntryKind, PointerKind, classify, create_pointer, remove_managed};

/// Lock file inside a managed root that serializes generation commits.
pub const COMMIT_LOCK_FILE_NAME: &str = dx_path::state::COMMIT_LOCK;

/// File beside the lock naming the command that last took it.
pub const COMMIT_HOLDER_FILE_NAME: &str = dx_path::state::COMMIT_LOCK_HOLDER;

/// Text for a busy workspace; names the holder when known.
#[must_use]
pub fn busy_message(path: &Path, holder: Option<&str>) -> String {
    format!(
        "workspace busy: {} holds {}; wait for it to finish, then retry",
        holder.unwrap_or("another dx command"),
        path.display()
    )
}

/// Deadline every domain waits for the commit lock before reporting busy.
pub const DEFAULT_COMMIT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Lock acquisition failure, without any domain policy attached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    #[error("{}", busy_message(path, holder.as_deref()))]
    Busy {
        path: PathBuf,
        holder: Option<String>,
    },
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
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        ManagedRoot { dir }
    }

    /// The root directory itself.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The lock file serializing this root.
    #[must_use]
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
    #[must_use]
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
        Ok(()) => {
            write_holder(root);
            Ok(CommitLock { file })
        }
        Err(std::fs::TryLockError::WouldBlock) => Err(LockError::Busy {
            path: path.clone(),
            holder: read_holder(root),
        }),
        // LCOV_EXCL_START - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        Err(e) => Err(LockError::LockFailed {
            path: path.clone(),
            reason: format!("cannot lock commit lock: {e}"),
        }),
        // LCOV_EXCL_STOP - reason: end non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

fn write_holder(root: &ManagedRoot) {
    let command = std::env::args_os()
        .nth(1)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let text = format!("dx {command} (pid {})", std::process::id());
    crate::write_hint(root.dir().join(COMMIT_HOLDER_FILE_NAME), text);
}

fn read_holder(root: &ManagedRoot) -> Option<String> {
    let mut bytes = std::fs::read(root.dir().join(COMMIT_HOLDER_FILE_NAME)).ok()?;
    bytes.truncate(200);
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(bytes.get(..error.valid_up_to())?).ok()?,
    };
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
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
        Err(e) if e.kind() == io::ErrorKind::NotFound && !under_a_file(stage) => Ok(()),
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

fn under_a_file(path: &Path) -> bool {
    path.ancestors().skip(1).any(Path::is_file)
}

/// Stages `target` and atomically publishes it; a failed rename keeps the prior pointer.
pub fn publish_pointer_swap(swap: &PointerSwap) -> Result<(), CommitError> {
    clear_stale_stage(&swap.stage)?;
    create_pointer(&swap.target, &swap.stage, swap.kind).map_err(|e| CommitError::Install {
        reason: format!("cannot stage {}: {e}", swap.stage.display()),
    })?;
    std::fs::rename(&swap.stage, &swap.current).map_err(|e| {
        drop(remove_managed(&swap.stage));
        CommitError::Install {
            reason: format!(
                "cannot publish {}: {e}; the prior pointer is preserved",
                swap.current.display()
            ),
        }
    })
}

/// One directory swap: `stage` replaces `current`, which moves to `retired` meanwhile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSwap {
    pub current: PathBuf,
    pub retired: PathBuf,
    pub stage: PathBuf,
}

/// Puts `retired` back at `current` when a crash fell between the two renames.
pub fn recover_tree_swap(swap: &TreeSwap) -> Result<(), CommitError> {
    let present = |path: &Path| classify(path).is_ok_and(|kind| kind != EntryKind::Missing);
    if !present(&swap.retired) || present(&swap.current) {
        return Ok(());
    }
    std::fs::rename(&swap.retired, &swap.current).map_err(|e| CommitError::Install {
        reason: format!("cannot restore interrupted tree: {e}"),
    })
}

/// Moves `current` aside, renames `stage` in and restores `current` when that fails.
pub fn publish_tree_swap(swap: &TreeSwap) -> Result<(), CommitError> {
    publish_tree_swap_with(swap, &|from, to| std::fs::rename(from, to))?;
    crate::discard_dir(&swap.retired);
    Ok(())
}

fn publish_tree_swap_with(
    swap: &TreeSwap,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<(), CommitError> {
    let retired = classify(&swap.current).is_ok_and(|kind| kind != EntryKind::Missing);
    if retired {
        rename(&swap.current, &swap.retired).map_err(|e| CommitError::Install {
            reason: format!("cannot retire current tree: {e}"),
        })?;
    }
    if let Err(e) = rename(&swap.stage, &swap.current) {
        let reason = match retired.then(|| rename(&swap.retired, &swap.current)) {
            Some(Err(e2)) => {
                format!("cannot publish staged tree: {e}; cannot restore previous tree: {e2}")
            }
            _ => format!("cannot publish staged tree: {e}"),
        };
        return Err(CommitError::Install { reason });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_at(dir: &Path) -> ManagedRoot {
        ManagedRoot::new(dir.to_path_buf())
    }

    #[test]
    fn second_acquire_reports_busy_with_the_lock_path() {
        let scratch = dx_test_scratch::scratch("dx-commit-lock-").unwrap();
        let _holding = dx_testing::holding();
        let root = root_at(&dx_path::state::local_dir(scratch.path()));
        root.ensure_dir().expect("root");
        assert_eq!(root.dir(), dx_path::state::local_dir(scratch.path()));
        assert_eq!(
            root.lock_path(),
            dx_path::state::local_dir(scratch.path()).join(COMMIT_LOCK_FILE_NAME)
        );
        let _held = acquire_commit_lock(&root, Duration::from_secs(10)).expect("hold commit lock");
        let busy = acquire_commit_lock(&root, Duration::ZERO).expect_err("locked root is busy");
        match &busy {
            LockError::Busy { path, holder } => {
                assert_eq!(path, &root.lock_path());
                let pid = format!("pid {}", std::process::id());
                assert!(holder.as_deref().expect("holder").contains(&pid));
            }
            other @ LockError::LockFailed { .. } => panic!("expected busy: {other:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        assert!(
            busy.to_string()
                .ends_with("wait for it to finish, then retry")
        );
        drop(_held);
        acquire_commit_lock(&root, Duration::ZERO).expect("released lock acquires");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn busy_without_a_holder_file_names_another_dx_command() {
        let scratch = dx_test_scratch::scratch("dx-commit-no-holder-").unwrap();
        let root = root_at(&dx_path::state::local_dir(scratch.path()));
        root.ensure_dir().expect("root");
        let _held = acquire_commit_lock(&root, Duration::from_secs(10)).expect("hold");
        std::fs::remove_file(root.dir().join(COMMIT_HOLDER_FILE_NAME)).expect("remove holder");
        let busy = acquire_commit_lock(&root, Duration::ZERO).expect_err("busy");
        assert!(busy.to_string().contains("another dx command"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn lock_open_failure_reports_the_path_and_keeps_foreign_state() {
        let scratch = dx_test_scratch::scratch("dx-commit-lock-open-").unwrap();
        let root = root_at(&dx_path::state::local_dir(scratch.path()));
        root.ensure_dir().expect("root");
        std::fs::create_dir_all(root.lock_path()).expect("lock is a directory");
        let error = acquire_commit_lock(&root, Duration::from_secs(1)).expect_err("open fails");
        match &error {
            LockError::LockFailed { path, reason } => {
                assert_eq!(path, &root.lock_path());
                assert_ne!(reason, "");
            }
            LockError::Busy { .. } => panic!("a directory lock file is not busy: {error:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        assert!(root.lock_path().is_dir(), "the foreign directory survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn root_ensure_refuses_a_blocking_file() {
        let scratch = dx_test_scratch::scratch("dx-commit-root-").unwrap();
        let blocking = dx_path::state::local_dir(scratch.path());
        std::fs::create_dir_all(dx_path::state::dx_dir(scratch.path())).expect("dx dir");
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
        let scratch = dx_test_scratch::scratch("dx-commit-handoff-").unwrap();
        let _holding = dx_testing::holding();
        let root = root_at(&dx_path::state::local_dir(scratch.path()));
        root.ensure_dir().expect("root");
        let lock = acquire_commit_lock(&root, Duration::from_secs(10)).expect("acquire");
        let file = lock.into_file();
        assert!(matches!(
            acquire_commit_lock(&root, Duration::ZERO).expect_err("handle stays locked"),
            LockError::Busy { .. }
        ));
        drop(file);
        acquire_commit_lock(&root, Duration::ZERO).expect("dropped handle releases");
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
        // LCOV_EXCL_START - reason: hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        probe
            .map_err(|_| {
                eprintln!("host refuses links; link cases are covered on hosts that allow them");
            })
            .is_ok()
        // LCOV_EXCL_STOP - reason: end hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }

    #[test]
    fn stale_stage_clear_matches_the_entry_kind() {
        let scratch = dx_test_scratch::scratch("dx-commit-stage-kinds-").unwrap();
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
        } // LCOV_EXCL_LINE - reason: implicit else on hosts that refuse links, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

        std::fs::write(&stage, "foreign").expect("foreign staged file");
        clear_stale_stage(&stage).expect("clear foreign staged file");
        assert!(stage.symlink_metadata().is_err(), "file removed");

        std::fs::create_dir_all(&stage).expect("foreign staged dir");
        let error = clear_stale_stage(&stage).expect_err("a staged directory stays refused");
        assert!(matches!(error, CommitError::Foreign { .. }));
        assert!(stage.is_dir(), "the refused directory survives");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn stale_stage_inside_a_file_reports_without_clearing() {
        let scratch = dx_test_scratch::scratch("dx-commit-stage-parent-").unwrap();
        let file = scratch.path().join("foreign");
        std::fs::write(&file, "foreign").expect("foreign file");
        let error = clear_stale_stage(&file.join("child")).expect_err("uninspectable stage fails");
        assert!(matches!(error, CommitError::Install { .. }));
        assert_eq!(std::fs::read(&file).expect("foreign preserved"), b"foreign");
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn stale_stage_in_a_read_only_directory_reports_without_clearing() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = dx_test_scratch::scratch("dx-commit-stage-readonly-").unwrap();
        let guarded = scratch.path().join("guarded");
        std::fs::create_dir_all(&guarded).expect("guarded dir");
        let stage = guarded.join("current.next");
        std::fs::write(&stage, "stale").expect("staged file");
        std::fs::set_permissions(&guarded, std::fs::Permissions::from_mode(0o555))
            .expect("chmod 555");
        let result = clear_stale_stage(&stage);
        std::fs::set_permissions(&guarded, std::fs::Permissions::from_mode(0o755))
            .expect("chmod back");
        // Root may delete from a read-only directory, so for root the stage is cleared.
        if dx_test_scratch::running_as_root() {
            result.expect("root clears the stage");
            assert!(!stage.exists(), "stage cleared");
        } else {
            let error = result.expect_err("unremovable stage fails");
            assert!(matches!(error, CommitError::Install { .. }));
            assert_eq!(
                std::fs::read(&stage).expect("refused stage survives"),
                b"stale"
            );
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(windows)]
    fn stale_stage_held_open_reports_without_clearing() {
        use std::os::windows::fs::OpenOptionsExt as _;
        let scratch = dx_test_scratch::scratch("dx-commit-stage-held-").unwrap();
        let stage = scratch.path().join("current.next");
        std::fs::write(&stage, "stale").expect("staged file");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&stage)
            .expect("hold the stage open");
        let error = clear_stale_stage(&stage).expect_err("a held stage fails");
        assert!(matches!(error, CommitError::Install { .. }));
        drop(held);
        assert_eq!(
            std::fs::read(&stage).expect("refused stage survives"),
            b"stale"
        );
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
        let scratch = dx_test_scratch::scratch("dx-commit-publish-fresh-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-commit-publish-idempotent-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-commit-publish-replace-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-commit-publish-stale-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-commit-publish-failure-").unwrap();
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        publish_pointer_swap(&swap_in(&dir, "record-a")).expect("first publish");
        let current = dir.join("current");
        dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
        std::fs::create_dir_all(&current).expect("blocking directory");
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
    fn publish_stage_failure_keeps_the_prior_pointer() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-stage-").unwrap();
        let dir = scratch.path().join("setups");
        std::fs::create_dir_all(&dir).expect("setups dir");
        publish_pointer_swap(&swap_in(&dir, "record-a")).expect("first publish");
        let broken = PointerSwap {
            current: dir.join("current"),
            stage: dir.join("absent-parent").join("current.next"),
            target: PathBuf::from("record-b"),
            kind: PointerKind::Directory,
        };
        let error = publish_pointer_swap(&broken).expect_err("staging without a parent fails");
        assert!(matches!(error, CommitError::Install { .. }));
        assert_eq!(
            std::fs::read_link(dir.join("current")).expect("prior pointer survives"),
            PathBuf::from("record-a")
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn commit_errors_display() {
        let errors = [
            CommitError::Foreign {
                path: PathBuf::from("/ws/.dx/local/current.next"),
                reason: "r".to_string(),
            },
            CommitError::Install {
                reason: "r".to_string(),
            },
        ];
        for error in &errors {
            assert_ne!(format!("{error}"), "");
        }
        let busy = LockError::Busy {
            path: PathBuf::from("/ws/.dx/local/.commit.lock"),
            holder: None,
        };
        assert_ne!(format!("{busy}"), "");
    }

    fn tree_swap_in(dir: &Path) -> TreeSwap {
        TreeSwap {
            current: dir.join("bin"),
            retired: dir.join("bin.prev"),
            stage: dir.join("bin.stage"),
        }
    }

    fn install_reason(error: CommitError) -> String {
        match error {
            CommitError::Install { reason } => reason,
            other @ CommitError::Foreign { .. } => panic!("expected install: {other:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
    }

    #[test]
    fn tree_swap_replaces_the_current_tree_and_drops_the_retired_one() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-").unwrap();
        let swap = tree_swap_in(scratch.path());
        std::fs::create_dir_all(&swap.current).expect("current");
        std::fs::write(swap.current.join("old"), "old").expect("old");
        std::fs::create_dir_all(&swap.stage).expect("stage");
        std::fs::write(swap.stage.join("new"), "new").expect("new");
        publish_tree_swap(&swap).expect("publish");
        assert!(swap.current.join("new").exists());
        assert!(!swap.current.join("old").exists());
        assert!(!swap.retired.exists());
        assert!(!swap.stage.exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn tree_swap_installs_a_fresh_tree() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-fresh-").unwrap();
        let swap = tree_swap_in(scratch.path());
        std::fs::create_dir_all(&swap.stage).expect("stage");
        publish_tree_swap(&swap).expect("publish");
        assert!(swap.current.is_dir());
        assert!(!swap.stage.exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn crash_restore_failure_reports_install() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-crash-fail-").unwrap();
        let root = scratch.path();
        let blocker = root.join("blocker");
        std::fs::write(&blocker, "file blocks dir").expect("blocker file");
        let swap = TreeSwap {
            current: blocker.join("bin"),
            retired: root.join("bin.prev"),
            stage: root.join("bin.stage"),
        };
        std::fs::create_dir_all(&swap.retired).expect("prev dir");
        let reason = install_reason(recover_tree_swap(&swap).unwrap_err());
        assert!(reason.contains("cannot restore interrupted tree"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn recover_keeps_a_present_current_tree() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-recover-").unwrap();
        let swap = tree_swap_in(scratch.path());
        recover_tree_swap(&swap).expect("nothing retired");
        std::fs::create_dir_all(&swap.current).expect("current");
        std::fs::create_dir_all(&swap.retired).expect("retired");
        recover_tree_swap(&swap).expect("current present");
        assert!(swap.current.is_dir());
        assert!(swap.retired.is_dir());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn tree_swap_failures_report_install() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-fail-").unwrap();
        let root = scratch.path();
        let swap = tree_swap_in(root);
        std::fs::create_dir_all(&swap.current).expect("bin dir");
        std::fs::create_dir_all(&swap.retired).expect("prev dir");
        std::fs::write(swap.retired.join("keep"), "keep").expect("occupied prev");
        std::fs::create_dir_all(&swap.stage).expect("stage dir");
        let reason = install_reason(publish_tree_swap(&swap).unwrap_err());
        assert!(reason.contains("cannot retire current tree"));

        let blocker = root.join("blocker");
        std::fs::write(&blocker, "file blocks dir").expect("blocker file");
        let blocked = TreeSwap {
            current: blocker.join("bin"),
            retired: root.join("other.prev"),
            stage: swap.stage.clone(),
        };
        let reason = install_reason(publish_tree_swap(&blocked).unwrap_err());
        assert!(reason.contains("cannot publish staged tree"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn tree_swap_publish_failure_restores_previous_tree() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-restore-").unwrap();
        let swap = tree_swap_in(scratch.path());
        std::fs::create_dir_all(&swap.current).expect("bin dir");
        std::fs::write(swap.current.join("old"), "old").expect("old");
        let reason = install_reason(publish_tree_swap(&swap).unwrap_err());
        assert!(
            reason.contains("cannot publish staged tree") && !reason.contains("cannot restore")
        );
        assert!(swap.current.join("old").exists());
        assert!(!swap.retired.exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn tree_swap_restore_failure_reports_both() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-both-").unwrap();
        let swap = tree_swap_in(scratch.path());
        std::fs::create_dir_all(&swap.current).expect("bin dir");
        std::fs::write(swap.current.join("old"), "old").expect("old");
        std::fs::create_dir_all(&swap.stage).expect("stage dir");
        let calls = std::cell::Cell::new(0u32);
        let rename = |a: &Path, b: &Path| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                std::fs::rename(a, b)
            } else {
                Err(io::Error::other("locked"))
            }
        };
        let reason = install_reason(publish_tree_swap_with(&swap, &rename).unwrap_err());
        assert!(
            reason.contains("cannot publish staged tree")
                && reason.contains("cannot restore previous tree")
        );
        assert!(!swap.current.exists());
        assert!(swap.retired.join("old").exists());
        recover_tree_swap(&swap).expect("recover");
        assert!(swap.current.join("old").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn busy_keeps_a_holder_label_cut_inside_a_character() {
        let scratch = dx_test_scratch::scratch("dx-commit-holder-cut-").unwrap();
        let root = root_at(&dx_path::state::local_dir(scratch.path()));
        root.ensure_dir().expect("root");
        let _held = acquire_commit_lock(&root, Duration::from_secs(10)).expect("hold");
        let label = format!("{}\u{e9}tail", "a".repeat(199));
        std::fs::write(root.dir().join(COMMIT_HOLDER_FILE_NAME), label).expect("holder");
        let busy = acquire_commit_lock(&root, Duration::ZERO).expect_err("busy");
        match busy {
            LockError::Busy { holder, .. } => assert_eq!(holder, Some("a".repeat(199))),
            other @ LockError::LockFailed { .. } => panic!("expected busy: {other:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn failed_publish_removes_its_staged_pointer() {
        let scratch = dx_test_scratch::scratch("dx-commit-publish-stage-left-").unwrap();
        let dir = scratch.path().join("setups");
        let swap = swap_in(&dir, "record-b");
        std::fs::create_dir_all(swap.current.join("kept")).expect("blocking directory");
        publish_pointer_swap(&swap).expect_err("rename must fail");
        assert!(
            std::fs::symlink_metadata(&swap.stage).is_err(),
            "stage left behind"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn tree_swap_replaces_a_dangling_link_at_current() {
        let scratch = dx_test_scratch::scratch("dx-tree-swap-dangling-").unwrap();
        let swap = tree_swap_in(scratch.path());
        std::os::unix::fs::symlink(scratch.path().join("gone"), &swap.current).expect("link");
        std::fs::create_dir_all(&swap.stage).expect("stage");
        std::fs::write(swap.stage.join("new"), "new").expect("new");
        publish_tree_swap(&swap).expect("publish over a dangling link");
        assert!(swap.current.join("new").exists());
        assert!(std::fs::symlink_metadata(&swap.retired).is_err());
        scratch.close().expect("cleanup");
    }
}
