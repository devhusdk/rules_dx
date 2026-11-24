//! Bounds the shared cache root: a test evicts the least recently used entries before its server starts.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// The byte limit of a shared root when `DX_NESTED_SHARED_LIMIT` does not set one.
pub const DEFAULT_LIMIT: u64 = 6 << 30;
const LOCK: &str = ".lock";
const WAIT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_secs(1);

/// A shared lock on the root, held while the server of this process may read the shared caches.
#[derive(Debug)]
pub struct Lease {
    _lock: File,
}

/// Takes a shared lease on `root`, first evicting the least recently used entries above `limit`.
///
/// Eviction needs the exclusive lock, so it never removes a repository a running test links to. An
/// over-limit root waits up to [`WAIT`] for the running tests to finish, then starts without evicting.
///
/// # Errors
///
/// Returns `Err` when the lock file cannot be opened or locked.
pub fn lease(root: &Path, limit: u64) -> Result<Lease, String> {
    std::fs::create_dir_all(root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let path = root.join(LOCK);
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let locked = |error: std::io::Error| format!("cannot lock {}: {error}", path.display());
    let started = Instant::now();
    let mut over = None;
    loop {
        match lock.try_lock() {
            Ok(()) => {
                evict(root, limit);
                lock.unlock().map_err(locked)?;
                break;
            }
            Err(TryLockError::WouldBlock) => {
                if !*over.get_or_insert_with(|| usage(root) > limit) || started.elapsed() > WAIT {
                    break;
                }
                std::thread::sleep(POLL);
            }
            Err(TryLockError::Error(error)) => return Err(locked(error)),
        }
    }
    lock.lock_shared().map_err(locked)?;
    Ok(Lease { _lock: lock })
}

/// The bytes of the files below `root`, links counted as themselves.
#[must_use]
pub fn usage(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => stack.push(entry.path()),
                Ok(meta) => total += meta.len(),
                Err(_) => {}
            }
        }
    }
    total
}

/// Removes whole entries, oldest use first, until `root` holds at most `limit` bytes.
pub fn evict(root: &Path, limit: u64) {
    let mut total = usage(root);
    if total <= limit {
        return;
    }
    let mut entries: Vec<(SystemTime, PathBuf)> = entries(root)
        .into_iter()
        .map(|path| (last_used(&path), path))
        .collect();
    entries.sort();
    for (_, path) in entries {
        if total <= limit {
            break;
        }
        let size = usage(&path);
        dx_process::make_writable(&path);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => total = total.saturating_sub(size),
            Err(error) => eprintln!("nested bazel: cannot evict {}: {error}", path.display()),
        }
    }
}

/// The evictable entries: install bases, fetched repositories and downloads of every cache set.
fn entries(root: &Path) -> Vec<PathBuf> {
    let mut buckets = vec![root.to_path_buf()];
    buckets.extend(dirs(&root.join(crate::SETS)));
    let mut parents = vec![root.join(crate::INSTALL)];
    for bucket in &buckets {
        parents.push(bucket.join(crate::REPO_CONTENTS));
        parents.extend(dirs(
            &bucket
                .join(crate::REPOSITORY_CACHE)
                .join("content_addressable"),
        ));
    }
    parents.iter().flat_map(|parent| dirs(parent)).collect()
}

fn dirs(parent: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

/// The newest modification time of the entries in `path`, or of `path` itself when empty.
///
/// Bazel stamps install base files in the future; those count as now.
fn last_used(path: &Path) -> SystemTime {
    let now = SystemTime::now();
    let modified = |meta: std::fs::Metadata| meta.modified().ok().map(|time| time.min(now));
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok().and_then(modified))
        .max()
        .or_else(|| std::fs::symlink_metadata(path).ok().and_then(modified))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(root: &Path, rel: &str, bytes: usize, age: Duration) -> PathBuf {
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = File::create(dir.join("f")).expect("create");
        file.set_len(bytes as u64).expect("size");
        file.set_modified(SystemTime::now() - age).expect("mtime");
        dir
    }

    #[test]
    fn eviction_drops_the_least_recently_used_entries_and_keeps_the_disk_cache() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = dir.path();
        let day = Duration::from_secs(86_400);
        let oldest = entry(root, "repo-contents/old", 100, day * 3);
        let older = entry(
            root,
            "sets/s/repository-cache/content_addressable/sha256/d",
            100,
            day * 2,
        );
        let fresh = entry(root, "repo-contents/new", 100, Duration::ZERO);
        let install = entry(root, "install/k", 10, Duration::ZERO);
        let disk = entry(root, "disk-cache/ac", 50, day * 9);
        assert_eq!(usage(root), 360);
        evict(root, 360);
        assert!(oldest.exists());
        evict(root, 260);
        assert!(!oldest.exists() && older.exists());
        evict(root, 160);
        assert!(!older.exists());
        assert!(fresh.exists() && install.exists() && disk.exists());
        assert_eq!(usage(root), 160);
    }

    #[test]
    fn a_lease_under_the_limit_evicts_nothing_and_holds_a_shared_lock() {
        let dir = tempfile::tempdir().expect("scratch");
        let kept = entry(dir.path(), "repo-contents/r", 100, Duration::ZERO);
        let first = lease(dir.path(), 1 << 20).expect("lease");
        let second = lease(dir.path(), 1 << 20).expect("second lease");
        let probe = File::options()
            .write(true)
            .open(dir.path().join(LOCK))
            .expect("open");
        assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
        drop((first, second));
        assert!(probe.try_lock().is_ok());
        assert!(kept.exists());
    }
}
