//! Bounds the shared cache root: a starting test evicts the least recently used entries no running test holds.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The byte limit of a shared root when `DX_NESTED_SHARED_LIMIT` does not set one.
pub const DEFAULT_LIMIT: u64 = 6 << 30;
/// Where an evicted entry moves before it is deleted, so a stopped delete never leaves half an entry in a cache.
///
/// Bazel's own repo contents GC uses the same name, so it is never an entry.
pub const TRASH: &str = "_trash";
const LOCK: &str = ".lock";
const LEASES: &str = "leases";
/// Covers coarse file system timestamps when a use is compared with the start of a lease.
const SLACK: Duration = Duration::from_secs(2);

/// The caches a test may read while its server runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holds {
    /// Its install base.
    pub install: PathBuf,
    /// Its repository cache, held whole because Bazel does not stamp the downloads it reads.
    pub downloads: PathBuf,
    /// Its repo contents cache, held for the entries used since the test started.
    pub fetched: PathBuf,
}

/// A running test's claim on its [`Holds`], released when its process exits.
#[derive(Debug)]
pub struct Lease {
    _lock: File,
}

#[derive(Debug)]
struct Held {
    since: SystemTime,
    holds: Holds,
}

impl Held {
    fn holds(&self, path: &Path, used: SystemTime) -> bool {
        path == self.holds.install
            || path.starts_with(&self.holds.downloads)
            || (path.starts_with(&self.holds.fetched) && used + SLACK >= self.since)
    }
}

/// Registers a lease on `holds` in `root` after evicting the least recently used entries above `limit`.
///
/// Registration and eviction run under an exclusive lock on the root, and eviction skips what running
/// tests hold, so a starting test trims the root without waiting for the others.
///
/// # Errors
///
/// Returns `Err` when the root or its lease files cannot be created or locked.
pub fn lease(root: &Path, limit: u64, holds: &Holds) -> Result<Lease, String> {
    let leases = root.join(LEASES);
    std::fs::create_dir_all(&leases)
        .map_err(|error| format!("cannot create {}: {error}", leases.display()))?;
    let guard_path = root.join(LOCK);
    let guard = open(&guard_path)?;
    guard
        .lock()
        .map_err(|error| format!("cannot lock {}: {error}", guard_path.display()))?;
    let held = running(&leases);
    let since = SystemTime::now();
    let nanos = since
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
        });
    let name = format!("{nanos}-{}", std::process::id());
    let lock_path = leases.join(format!("{name}.lock"));
    let lock = open(&lock_path)?;
    lock.try_lock()
        .map_err(|error| format!("cannot lock {}: {error}", lock_path.display()))?;
    let record = leases.join(&name);
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{} is not UTF-8", path.display()))
    };
    let lines = [
        nanos.to_string(),
        text(&holds.install)?,
        text(&holds.downloads)?,
        text(&holds.fetched)?,
    ];
    std::fs::write(&record, lines.join("\n"))
        .map_err(|error| format!("cannot write {}: {error}", record.display()))?;
    evict(root, limit, &held, &name);
    guard
        .unlock()
        .map_err(|error| format!("cannot unlock {}: {error}", guard_path.display()))?;
    drop(guard);
    empty_trash(root);
    Ok(Lease { _lock: lock })
}

fn open(path: &Path) -> Result<File, String> {
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))
}

/// The holds of the leases whose process still runs; the records of finished ones are removed.
fn running(leases: &Path) -> Vec<Held> {
    let mut held = Vec::new();
    for entry in std::fs::read_dir(leases).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "lock") {
            continue;
        }
        let record = path.with_extension("");
        let Ok(lock) = File::options().write(true).open(&path) else {
            continue;
        };
        match lock.try_lock() {
            Ok(()) => {
                drop(lock);
                for stale in [&record, &path] {
                    if let Err(error) = std::fs::remove_file(stale)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        eprintln!("nested bazel: cannot remove {}: {error}", stale.display());
                    }
                }
            }
            Err(TryLockError::WouldBlock) => held.extend(read_record(&record)),
            Err(TryLockError::Error(error)) => {
                eprintln!("nested bazel: cannot probe {}: {error}", path.display());
            }
        }
    }
    held
}

fn read_record(path: &Path) -> Option<Held> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let since = SystemTime::UNIX_EPOCH + Duration::from_nanos(lines.next()?.parse().ok()?);
    let mut next = || lines.next().map(PathBuf::from);
    Some(Held {
        since,
        holds: Holds {
            install: next()?,
            downloads: next()?,
            fetched: next()?,
        },
    })
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

/// Moves unheld entries, oldest use first, into the trash until `root` holds at most `limit` bytes.
///
/// An entry that cannot move, such as one a lingering Windows server keeps open, stays whole.
fn evict(root: &Path, limit: u64, held: &[Held], tag: &str) {
    let trash = root.join(TRASH);
    let mut total = usage(root).saturating_sub(usage(&trash));
    if total <= limit {
        return;
    }
    let install = root.join(crate::INSTALL);
    let mut entries: Vec<(SystemTime, PathBuf)> = entries(root)
        .into_iter()
        .map(|path| (last_used(&path, path.parent() == Some(&install)), path))
        .filter(|(used, path)| !held.iter().any(|lease| lease.holds(path, *used)))
        .collect();
    entries.sort();
    if let Err(error) = std::fs::create_dir_all(&trash) {
        eprintln!("nested bazel: cannot create {}: {error}", trash.display());
        return;
    }
    for (index, (_, path)) in entries.into_iter().enumerate() {
        if total <= limit {
            break;
        }
        let size = usage(&path);
        match std::fs::rename(&path, trash.join(format!("{tag}-{index}"))) {
            Ok(()) => total = total.saturating_sub(size),
            Err(error) => eprintln!("nested bazel: cannot evict {}: {error}", path.display()),
        }
    }
}

fn empty_trash(root: &Path) {
    for path in dirs(&root.join(TRASH)) {
        dx_process::make_writable(&path);
        if let Err(error) = std::fs::remove_dir_all(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("nested bazel: cannot delete {}: {error}", path.display());
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
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir()) && entry.file_name() != TRASH
        })
        .map(|entry| entry.path())
        .collect()
}

/// When `path` was last used: its own mtime when `own`, else the newest of it and its children.
///
/// The Bazel client stamps an install base directory at each start but its files in the future, so an
/// install base passes `own`; future stamps count as now.
fn last_used(path: &Path, own: bool) -> SystemTime {
    let now = SystemTime::now();
    let modified = |meta: std::fs::Metadata| meta.modified().ok().map(|time| time.min(now));
    let itself = std::fs::symlink_metadata(path).ok().and_then(modified);
    let children = std::fs::read_dir(path)
        .into_iter()
        .filter(|_| !own)
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok().and_then(modified));
    itself
        .into_iter()
        .chain(children)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(86_400);

    fn entry(root: &Path, rel: &str, bytes: u64, age: Duration) -> PathBuf {
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = File::create(dir.join("f")).expect("create");
        file.set_len(bytes).expect("size");
        file.set_modified(SystemTime::now() - age).expect("mtime");
        backdate(&dir, age);
        dir
    }

    fn backdate(dir: &Path, age: Duration) {
        #[cfg(windows)]
        let handle = {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            File::options()
                .access_mode(FILE_WRITE_ATTRIBUTES)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(dir)
        };
        #[cfg(not(windows))]
        let handle = File::open(dir);
        handle
            .expect("open dir")
            .set_modified(SystemTime::now() - age)
            .expect("dir mtime");
    }

    fn holds(root: &Path, install: &str, bucket: &str) -> Holds {
        let bucket = root.join(bucket);
        Holds {
            install: root.join("install").join(install),
            downloads: bucket.join("repository-cache"),
            fetched: bucket.join("repo-contents"),
        }
    }

    fn leases(root: &Path) -> usize {
        std::fs::read_dir(root.join(LEASES))
            .expect("leases")
            .count()
    }

    #[test]
    fn eviction_drops_the_least_recently_used_entries_and_keeps_the_disk_cache() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = dir.path();
        let oldest = entry(root, "repo-contents/old", 100, DAY * 3);
        let older = entry(
            root,
            "sets/s/repository-cache/content_addressable/sha256/d",
            100,
            DAY * 2,
        );
        let fresh = entry(root, "repo-contents/new", 100, Duration::ZERO);
        let install = entry(root, "install/k", 10, Duration::ZERO);
        let disk = entry(root, "disk-cache/ac", 50, DAY * 9);
        assert_eq!(usage(root), 360);
        evict(root, 360, &[], "t");
        assert!(oldest.exists());
        evict(root, 260, &[], "t");
        assert!(!oldest.exists() && older.exists());
        evict(root, 160, &[], "u");
        assert!(!older.exists());
        assert!(fresh.exists() && install.exists() && disk.exists());
        empty_trash(root);
        assert_eq!(usage(root), 160);
    }

    #[test]
    fn an_unused_install_base_goes_before_recent_repositories_despite_its_future_stamps() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = dir.path();
        let install = root.join("install/k");
        std::fs::create_dir_all(&install).expect("mkdir");
        let file = File::create(install.join("f")).expect("create");
        file.set_len(100).expect("size");
        file.set_modified(SystemTime::now() + DAY * 3650)
            .expect("mtime");
        backdate(&install, DAY * 3);
        let fresh = entry(root, "repo-contents/new", 100, Duration::ZERO);
        evict(root, 100, &[], "t");
        assert!(!install.exists() && fresh.exists());
    }

    #[test]
    fn a_running_lease_keeps_its_install_base_downloads_and_used_repositories() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = dir.path();
        let install = entry(root, "install/a", 100, DAY * 3);
        let download = entry(
            root,
            "sets/s/repository-cache/content_addressable/sha256/d",
            100,
            DAY * 3,
        );
        let unused = entry(root, "sets/s/repo-contents/unused", 100, DAY * 3);
        let other = entry(root, "repo-contents/other", 100, DAY * 2);
        let first = lease(root, u64::MAX, &holds(root, "a", "sets/s")).expect("lease");
        let used = entry(root, "sets/s/repo-contents/used", 100, Duration::ZERO);
        let second = lease(root, 0, &holds(root, "b", "")).expect("second lease");
        assert!(install.exists() && download.exists() && used.exists());
        assert!(!unused.exists() && !other.exists());
        assert!(dirs(&root.join(TRASH)).is_empty());
        assert_eq!(leases(root), 4);
        drop((first, second));
        let _third = lease(root, 0, &holds(root, "c", "")).expect("third lease");
        assert!(!install.exists() && !download.exists() && !used.exists());
        assert_eq!(leases(root), 2);
    }

    #[test]
    fn an_interrupted_eviction_leaves_whole_entries_and_its_trash_is_emptied() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = dir.path();
        let half = entry(root, "_trash/1-2-0/half", 100, DAY);
        let bazel_trash = entry(root, "repo-contents/_trash/x", 100, DAY * 9);
        let repository = entry(root, "repo-contents/r", 100, DAY);
        assert!(!entries(root).contains(&root.join("repo-contents/_trash")));
        let _lease = lease(root, u64::MAX, &holds(root, "a", "")).expect("lease");
        assert!(!half.exists() && bazel_trash.exists() && repository.exists());
    }
}
