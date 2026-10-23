//! Leases that let the next dx run stop process groups a dead dx left behind.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const STALE_EMPTY: Duration = Duration::from_secs(60);
const REAP_GRACE: Duration = Duration::from_secs(2);

static LEASE_DIR: OnceLock<PathBuf> = OnceLock::new();
static SEQUENCE: AtomicU32 = AtomicU32::new(0);
static KEPT: Mutex<Vec<File>> = Mutex::new(Vec::new());

/// One process group stopped by [`reap_orphans`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaped {
    pub pid: u32,
    pub pgid: i32,
}

/// Sets the directory that holds this worktree's group leases; the first call wins.
pub fn set_lease_dir(dir: PathBuf) {
    let _ = LEASE_DIR.set(dir);
}

/// A held lease for one owned process group; dropping removes it once the group is gone.
pub(crate) struct GroupLease {
    file: Option<File>,
    path: PathBuf,
    pgid: i32,
}

impl GroupLease {
    pub(crate) fn acquire() -> Option<GroupLease> {
        let dir = LEASE_DIR.get()?;
        std::fs::create_dir_all(dir).ok()?;
        let name = format!(
            "{}-{}.lock",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        );
        let path = dir.join(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .ok()?;
        file.try_lock().ok()?;
        Some(GroupLease {
            file: Some(file),
            path,
            pgid: 0,
        })
    }

    pub(crate) fn record(&mut self, pgid: i32) {
        self.pgid = pgid;
        if let Some(mut file) = self.file.as_ref() {
            let _ = writeln!(file, "{pgid}");
            let _ = file.flush();
        }
    }
}

impl Drop for GroupLease {
    fn drop(&mut self) {
        let Some(file) = self.file.take() else {
            return;
        };
        if self.pgid != 0 && group_alive(self.pgid) {
            if let Ok(mut kept) = KEPT.lock() {
                kept.push(file);
                return;
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
mod sys {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    pub fn group_alive(pgid: i32) -> bool {
        pgid > 1 && unsafe { kill(-pgid, 0) } == 0
    }

    pub fn signal_group(pgid: i32, signo: i32) {
        if pgid > 1 {
            unsafe {
                kill(-pgid, signo);
            }
        }
    }
}

#[cfg(not(unix))]
mod sys {
    pub fn group_alive(_pgid: i32) -> bool {
        false
    }

    pub fn signal_group(_pgid: i32, _signo: i32) {}
}

fn group_alive(pgid: i32) -> bool {
    sys::group_alive(pgid)
}

/// Stops every recorded group whose dx is gone, and returns the groups it stopped.
pub fn reap_orphans() -> Vec<Reaped> {
    match LEASE_DIR.get() {
        Some(dir) => reap_in(dir),
        None => Vec::new(),
    }
}

fn reap_in(dir: &Path) -> Vec<Reaped> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut reaped = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(pid) = lease_pid(&path) else {
            continue;
        };
        let Ok(mut file) = OpenOptions::new().read(true).write(true).open(&path) else {
            continue;
        };
        if file.try_lock().is_err() {
            continue;
        }
        let mut text = String::new();
        let _ = file.read_to_string(&mut text);
        match text.trim().parse::<i32>() {
            Ok(pgid) => {
                if group_alive(pgid) {
                    stop_group(pgid);
                    reaped.push(Reaped { pid, pgid });
                }
                let _ = std::fs::remove_file(&path);
            }
            Err(_) => {
                let old = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|time| time.elapsed().ok())
                    .is_some_and(|age| age >= STALE_EMPTY);
                if old {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    reaped.sort_by_key(|group| group.pgid);
    reaped
}

fn lease_pid(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".lock")?;
    stem.split_once('-')?.0.parse().ok()
}

fn stop_group(pgid: i32) {
    const SIGTERM: i32 = 15;
    const SIGKILL: i32 = 9;
    sys::signal_group(pgid, SIGTERM);
    let deadline = std::time::Instant::now() + REAP_GRACE;
    while group_alive(pgid) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if group_alive(pgid) {
        sys::signal_group(pgid, SIGKILL);
    }
}

/// The one line printed for a stopped group.
pub fn reaped_message(group: &Reaped) -> String {
    format!(
        "stopped leftover process group {} from dx run {}, which is gone",
        group.pgid, group.pid
    )
}

#[cfg(test)]
#[path = "groups_tests.rs"]
mod groups_tests;
