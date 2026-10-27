//! Leases that let the next dx run stop process groups a dead dx left behind.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

const STALE_EMPTY: Duration = Duration::from_secs(60);
const REAP_GRACE: Duration = Duration::from_secs(2);
const GROUPS_DIR_NAME: &str = "process-groups";

static LEASE_DIR: OnceLock<PathBuf> = OnceLock::new();
static SEQUENCE: AtomicU32 = AtomicU32::new(0);

/// One process group stopped by [`reap_orphans`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaped {
    pub pid: u32,
    pub pgid: i32,
}

/// Sets the managed root that holds this worktree's group leases; the first call wins.
pub fn set_lease_root(root: &Path) {
    let _ = LEASE_DIR.set(
        root.join(dx_atomic_fs::lease::LEASES_DIR_NAME)
            .join(GROUPS_DIR_NAME),
    );
}

/// A held lease for one owned process group; dropping removes it.
pub(crate) struct GroupLease {
    file: File,
    path: PathBuf,
}

/// The directory set by [`set_lease_root`], if any.
pub(crate) fn lease_dir() -> Option<&'static Path> {
    LEASE_DIR.get().map(PathBuf::as_path)
}

impl GroupLease {
    pub(crate) fn acquire_in(dir: &Path) -> Option<GroupLease> {
        let name = format!(
            "{}-{}.lock",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        );
        let path = dir.join(name);
        let file = dx_atomic_fs::lease::try_lock_file(&path, true)
            .ok()
            .flatten()?;
        Some(GroupLease { file, path })
    }

    pub(crate) fn record(&mut self, pgid: i32) {
        let _ = self.file.set_len(0);
        let _ = self.file.seek(SeekFrom::Start(0));
        let _ = writeln!(self.file, "{}", lease_line(pgid));
        let _ = self.file.flush();
    }
}

impl Drop for GroupLease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) fn lease_line(pgid: i32) -> String {
    format!(
        "{pgid} {} {}",
        sys::namespace(),
        sys::start_time(pgid).unwrap_or_else(|| "-".to_owned())
    )
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

    #[cfg(target_os = "linux")]
    pub fn namespace() -> String {
        std::fs::read_link("/proc/self/ns/pid")
            .map(|link| link.display().to_string())
            .unwrap_or_else(|_| "-".to_owned())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn namespace() -> String {
        "-".to_owned()
    }

    #[cfg(target_os = "linux")]
    pub fn start_time(pid: i32) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let rest = stat.rsplit_once(')')?.1;
        rest.split_whitespace().nth(19).map(str::to_owned)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn start_time(pid: i32) -> Option<String> {
        let text = crate::probe("ps", ["-o", "lstart=", "-p", &pid.to_string()])?;
        let text = text.split_whitespace().collect::<Vec<_>>().join("_");
        (!text.is_empty()).then_some(text)
    }
}

#[cfg(not(unix))]
mod sys {
    pub fn group_alive(_pgid: i32) -> bool {
        false
    }

    pub fn signal_group(_pgid: i32, _signo: i32) {}

    pub fn namespace() -> String {
        "-".to_owned()
    }

    pub fn start_time(_pid: i32) -> Option<String> {
        None
    }
}

fn group_alive(pgid: i32) -> bool {
    sys::group_alive(pgid)
}

/// Stops every recorded group whose dx is gone, and returns the groups it stopped.
pub fn reap_orphans() -> Vec<Reaped> {
    match lease_dir() {
        Some(dir) => reap_in(dir),
        None => Vec::new(),
    }
}

struct Recorded {
    pgid: i32,
    namespace: String,
    start: String,
}

fn parse_lease(text: &str) -> Option<Recorded> {
    let mut parts = text.split_whitespace();
    Some(Recorded {
        pgid: parts.next()?.parse().ok()?,
        namespace: parts.next()?.to_owned(),
        start: parts.next()?.to_owned(),
    })
}

fn same_group(recorded: &Recorded) -> bool {
    if recorded.start == "-" {
        return true;
    }
    match sys::start_time(recorded.pgid) {
        Some(now) => now == recorded.start,
        None => true,
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
        let Ok(Some(mut file)) = dx_atomic_fs::lease::try_lock_file(&path, false) else {
            continue;
        };
        let mut text = String::new();
        let _ = file.read_to_string(&mut text);
        match parse_lease(&text) {
            Some(recorded) => {
                if recorded.namespace != sys::namespace() {
                    continue;
                }
                if group_alive(recorded.pgid) && same_group(&recorded) {
                    stop_group(recorded.pgid);
                    reaped.push(Reaped {
                        pid,
                        pgid: recorded.pgid,
                    });
                }
                let _ = std::fs::remove_file(&path);
            }
            None => {
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
    let stem = name.strip_suffix(dx_atomic_fs::lease::LEASE_FILE_SUFFIX)?;
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

/// Notice code for a leftover process group dx stopped at startup.
pub const CODE_ORPHAN_PROCESS_GROUP_STOPPED: &str = "orphan_process_group_stopped";

/// The one line printed for a stopped group.
pub fn reaped_message(group: &Reaped) -> String {
    format!(
        "stopped leftover process group {} from dx run {}, which is gone",
        group.pgid, group.pid
    )
}

#[cfg(test)]
#[cfg(unix)]
#[path = "groups_tests.rs"]
mod groups_tests;
