use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

pub fn bep_path(temp_dir: &Path, pid: u32, nonce: u64) -> PathBuf {
    temp_dir.join(format!("dx-bep-{pid}-{nonce}.json"))
}

pub fn results_path(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join("results.json")
}

/// Identifies one run: `{pid}-{nonce}-{suffix}`, the suffix being the random part of the
/// `dx-run-{pid}-XXXXXX` temp dir name (omitted when the dir has another name).
pub fn run_id(pid: u32, nonce: u64, temp_dir: &Path) -> String {
    let suffix = temp_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_prefix(&format!("dx-run-{pid}-"))
                .or_else(|| name.strip_prefix("dx-run-"))
        })
        .filter(|suffix| !suffix.is_empty());
    match suffix {
        Some(suffix) => format!("{pid}-{nonce}-{suffix}"),
        None => format!("{pid}-{nonce}"),
    }
}

pub fn intended_path(temp_dir: &Path, pid: u32, nonce: u64) -> PathBuf {
    temp_dir.join(format!("dx-generate-{pid}-{nonce}.json"))
}

pub fn patch_path(temp_dir: &Path, pid: u32, nonce: u64) -> PathBuf {
    temp_dir.join(format!("dx-generate-{pid}-{nonce}.patch"))
}

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn run_nonce() -> u64 {
    RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
}

const OWNER_LOCK: &str = "owner.lock";

/// A run temp dir whose owner holds an exclusive lock on `owner.lock` until it closes.
pub struct RunTempDir {
    dir: tempfile::TempDir,
    owner: Option<std::fs::File>,
}

impl RunTempDir {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Releases the owner lock and removes the directory, tolerating a sweeper that won the race.
    pub fn close(self) -> std::io::Result<()> {
        drop(self.owner);
        match self.dir.close() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

pub fn create_run_temp_dir(base: &Path, pid: u32) -> std::io::Result<(RunTempDir, u64)> {
    let dir = tempfile::Builder::new()
        .prefix(&format!("dx-run-{pid}-"))
        .tempdir_in(base)?;
    let owner = dx_atomic_fs::lease::try_lock_file(&dir.path().join(OWNER_LOCK), true)
        .ok()
        .flatten();
    let nonce = run_nonce();
    Ok((RunTempDir { dir, owner }, nonce))
}

fn run_dir_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("dx-run-")?;
    let (pid, suffix) = rest.split_once('-')?;
    if suffix.is_empty() || pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

/// Removes run dirs whose dx owner is gone.
pub fn remove_stale_run_dirs(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_str().and_then(run_dir_pid).is_none() {
            continue;
        }
        let path = entry.path();
        if let Ok(Some(lock)) = dx_atomic_fs::lease::try_lock_file(&path.join(OWNER_LOCK), false) {
            drop(lock);
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

static STOP_CLEANUP: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Registers a directory to remove when a stop signal ends dx.
pub fn remove_on_stop(path: &Path) {
    *STOP_CLEANUP.lock().unwrap_or_else(PoisonError::into_inner) = Some(path.to_path_buf());
}

/// Removes the registered directory and returns the held lock.
pub fn clean_for_stop() -> MutexGuard<'static, Option<PathBuf>> {
    let mut slot = STOP_CLEANUP.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(path) = slot.take() {
        let _ = std::fs::remove_dir_all(path);
    }
    slot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_is_stable_per_run_and_differs_by_nonce() {
        let dir = Path::new("/tmp/dx-run-7-AbC123");
        assert_eq!(run_id(7, 1, dir), "7-1-AbC123");
        assert_eq!(run_id(7, 1, Path::new("/tmp/dx-run-AbC123")), "7-1-AbC123");
        assert_eq!(run_id(7, 1, dir), run_id(7, 1, dir));
        assert_ne!(run_id(7, 1, dir), run_id(7, 2, dir));
        assert_eq!(run_id(7, 1, Path::new("/tmp/harness")), "7-1");
    }

    #[test]
    fn clean_for_stop_removes_the_registered_run_dir_once() {
        let (dir, nonce) = create_run_temp_dir(&std::env::temp_dir(), 7).expect("run dir");
        let path = dir.path().to_path_buf();
        std::fs::write(bep_path(&path, 7, nonce), b"{}").expect("bep file");
        remove_on_stop(&path);
        drop(clean_for_stop());
        assert!(!path.exists(), "{} must be removed", path.display());
        let second = clean_for_stop();
        assert!(second.is_none(), "a second cleanup must be a no-op");
        drop(second);
        drop(dir);
    }

    #[test]
    fn remove_stale_run_dirs_keeps_live_and_removes_gone_owners() {
        let base = dx_test_scratch::scratch("dx-run-sweep-");
        let base = base.path();
        let gone = base.join("dx-run-999999-abc");
        std::fs::create_dir(&gone).expect("gone dir");
        std::fs::write(gone.join(OWNER_LOCK), b"").expect("gone lock");
        std::fs::write(gone.join("dx-bep-1-0.json"), b"{}").expect("gone file");
        let (live, _) = create_run_temp_dir(base, std::process::id()).expect("live dir");
        let unlocked = base.join("dx-run-1-nolock");
        std::fs::create_dir(&unlocked).expect("nolock dir");
        let old_format = base.join("dx-run-abcdef");
        std::fs::create_dir(&old_format).expect("old dir");
        let bep = base.join("dx-bep-1-0.json");
        std::fs::write(&bep, b"{}").expect("bep");
        remove_stale_run_dirs(base);
        assert!(!gone.exists(), "a dir with an unlocked owner must go");
        assert!(live.path().is_dir(), "a held dir must stay");
        assert!(unlocked.is_dir(), "a dir without owner.lock must stay");
        assert!(old_format.is_dir(), "an old-format dir must stay");
        assert!(bep.is_file(), "other dx files must stay");
        live.close().expect("close live");
    }

    #[test]
    fn close_tolerates_a_dir_already_removed() {
        let (dir, _) = create_run_temp_dir(&std::env::temp_dir(), 7).expect("run dir");
        std::fs::remove_dir_all(dir.path()).expect("swept");
        dir.close().expect("close after sweep");
    }

    #[test]
    fn bep_paths_are_unique_per_run() {
        let dir = Path::new("/tmp/dx");
        assert_eq!(
            bep_path(dir, 42, 0),
            PathBuf::from("/tmp/dx/dx-bep-42-0.json")
        );
        assert_ne!(bep_path(dir, 42, 0), bep_path(dir, 42, 1));
        assert_ne!(bep_path(dir, 42, 0), bep_path(dir, 43, 0));
    }

    #[test]
    fn run_nonces_are_unique_in_process() {
        use std::collections::HashSet;
        let seen: HashSet<u64> = (0..512).map(|_| run_nonce()).collect();
        assert_eq!(seen.len(), 512, "counter-backed nonces must not repeat");
    }

    #[test]
    fn create_run_temp_dir_is_unique_and_real() {
        let base = std::env::temp_dir();
        let (first, first_nonce) = create_run_temp_dir(&base, 7).expect("first run dir");
        let (second, second_nonce) = create_run_temp_dir(&base, 7).expect("second run dir");
        assert_ne!(
            first.path(),
            second.path(),
            "concurrent runs must never share a directory"
        );
        assert_ne!(first_nonce, second_nonce);
        assert!(first.path().is_dir() && second.path().is_dir());
        assert_eq!(first.path().parent(), Some(base.as_path()));
        assert_eq!(second.path().parent(), Some(base.as_path()));
        for dir in [&first, &second] {
            let name = dir
                .path()
                .file_name()
                .expect("run dir has a file name")
                .to_string_lossy();
            assert!(
                name.starts_with("dx-run-7-"),
                "run dir {name:?} must carry the dx-run-7- prefix"
            );
            assert!(dir.path().join(OWNER_LOCK).is_file());
        }
        first.close().expect("cleanup first");
        second.close().expect("cleanup second");
    }
}
