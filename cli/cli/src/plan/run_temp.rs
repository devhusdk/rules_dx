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
/// `dx-run-XXXXXX` temp dir name (omitted when the dir has another name).
pub fn run_id(pid: u32, nonce: u64, temp_dir: &Path) -> String {
    let suffix = temp_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("dx-run-"))
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

pub fn create_run_temp_dir(base: &Path) -> std::io::Result<(tempfile::TempDir, u64)> {
    let dir = tempfile::Builder::new()
        .prefix("dx-run-")
        .tempdir_in(base)?;
    let nonce = run_nonce();
    Ok((dir, nonce))
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
        let dir = Path::new("/tmp/dx-run-AbC123");
        assert_eq!(run_id(7, 1, dir), "7-1-AbC123");
        assert_eq!(run_id(7, 1, dir), run_id(7, 1, dir));
        assert_ne!(run_id(7, 1, dir), run_id(7, 2, dir));
        assert_eq!(run_id(7, 1, Path::new("/tmp/harness")), "7-1");
    }

    #[test]
    fn clean_for_stop_removes_the_registered_run_dir_once() {
        let (dir, nonce) = create_run_temp_dir(&std::env::temp_dir()).expect("run dir");
        let path = dir.keep();
        std::fs::write(bep_path(&path, 7, nonce), b"{}").expect("bep file");
        remove_on_stop(&path);
        drop(clean_for_stop());
        assert!(!path.exists(), "{} must be removed", path.display());
        let second = clean_for_stop();
        assert!(second.is_none(), "a second cleanup must be a no-op");
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
        let (first, first_nonce) = create_run_temp_dir(&base).expect("first run dir");
        let (second, second_nonce) = create_run_temp_dir(&base).expect("second run dir");
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
                name.starts_with("dx-run-"),
                "run dir {name:?} must carry the dx-run- prefix"
            );
        }
        first.close().expect("cleanup first");
        second.close().expect("cleanup second");
    }
}
