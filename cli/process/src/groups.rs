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
        let Some(line) = lease_line(pgid) else {
            return;
        };
        let _ = writeln!(self.file, "{line}");
        let _ = self.file.flush();
    }
}

impl Drop for GroupLease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The lease text for a group, or `None` when its start time is unknown: such a line could
/// never prove the group is still the recorded one, so nothing is written.
pub(crate) fn lease_line(pgid: i32) -> Option<String> {
    let start = sys::start_time(pgid)?;
    Some(format!(
        "{pgid} {} {start} {}",
        sys::namespace(),
        sys::boot_id()
    ))
}

/// What is known about the process whose pid equals a recorded group number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Leader {
    Present(String),
    Absent,
    Unknown,
}

#[cfg(unix)]
mod sys {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    unsafe extern "C" {
        fn getpgrp() -> i32;
        fn getsid(pid: i32) -> i32;
        fn getppid() -> i32;
        fn getpgid(pid: i32) -> i32;
    }

    pub fn group_alive(pgid: i32) -> bool {
        pgid > 1 && unsafe { kill(-pgid, 0) } == 0
    }

    /// Groups the running dx belongs to, which it must never stop.
    pub fn own_groups() -> Vec<i32> {
        unsafe { vec![getpgrp(), getsid(0), getpgid(getppid())] }
    }

    #[cfg(target_os = "linux")]
    pub fn boot_id() -> String {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "-".to_owned())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn boot_id() -> String {
        "-".to_owned()
    }

    #[cfg(target_os = "linux")]
    pub fn leader(pid: i32) -> super::Leader {
        use super::Leader;
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().nth(19).map(str::to_owned))
                .map_or(Leader::Unknown, Leader::Present),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Leader::Absent,
            Err(_) => Leader::Unknown,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn leader(pid: i32) -> super::Leader {
        use super::Leader;
        const ESRCH: i32 = 3;
        if unsafe { kill(pid, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(ESRCH)
        {
            return Leader::Absent;
        }
        start_time(pid).map_or(Leader::Unknown, Leader::Present)
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

    pub fn own_groups() -> Vec<i32> {
        Vec::new()
    }

    pub fn boot_id() -> String {
        "-".to_owned()
    }

    pub fn leader(_pid: i32) -> super::Leader {
        super::Leader::Unknown
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
    boot: Option<String>,
}

fn parse_lease(text: &str) -> Option<Recorded> {
    let mut parts = text.split_whitespace();
    Some(Recorded {
        pgid: parts.next()?.parse().ok()?,
        namespace: parts.next()?.to_owned(),
        start: parts.next()?.to_owned(),
        boot: parts.next().map(str::to_owned),
    })
}

/// What the reaper sees right now; gathered by syscalls so [`verdict`] can stay pure.
#[derive(Debug, Clone)]
struct Observed {
    namespace: String,
    group_alive: bool,
    leader: Leader,
    boot: String,
    own: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Signal the group.
    Stop,
    /// Forget the lease without signalling anything.
    Drop,
    /// Leave the lease for a later run.
    Keep,
}

/// Decides whether a recorded group may be stopped. It signals only when the group is
/// provably the recorded one; anything unproven never becomes `Stop`.
fn verdict(recorded: &Recorded, now: &Observed) -> Verdict {
    if recorded.namespace != now.namespace {
        return Verdict::Keep;
    }
    if cfg!(target_os = "linux") && recorded.namespace == "-" {
        return Verdict::Drop;
    }
    if !now.group_alive || recorded.start == "-" {
        return Verdict::Drop;
    }
    if let Some(boot) = &recorded.boot {
        if boot != "-" && now.boot != "-" && *boot != now.boot {
            return Verdict::Drop;
        }
    }
    if now.own.contains(&recorded.pgid) {
        return Verdict::Drop;
    }
    match &now.leader {
        Leader::Present(start) if *start == recorded.start => Verdict::Stop,
        Leader::Present(_) => Verdict::Drop,
        Leader::Unknown => Verdict::Keep,
        // A pid is not reused while its group exists, so a live group with a gone leader
        // is still the recorded one.
        Leader::Absent => Verdict::Stop,
    }
}

fn observe(recorded: &Recorded) -> Observed {
    Observed {
        namespace: sys::namespace(),
        group_alive: group_alive(recorded.pgid),
        leader: sys::leader(recorded.pgid),
        boot: sys::boot_id(),
        own: sys::own_groups(),
    }
}

const LOG_NAME: &str = "reaped.log";
const LOG_CAP: u64 = 64 * 1024;

/// Appends one decision line to `reaped.log`; errors are ignored so startup is never slowed.
fn log_decision(dir: &Path, what: &str, pid: u32, recorded: &Recorded, now: &Observed) {
    let log = dir.join(LOG_NAME);
    if std::fs::metadata(&log).is_ok_and(|meta| meta.len() > LOG_CAP) {
        let _ = std::fs::rename(&log, dir.join(format!("{LOG_NAME}.1")));
    }
    let cmdline = std::fs::read(format!("/proc/{}/cmdline", recorded.pgid))
        .map(|bytes| {
            let text = String::from_utf8_lossy(&bytes).replace('\0', " ");
            text.chars().take(200).collect::<String>()
        })
        .unwrap_or_default();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let line = format!(
        "{secs} {what} reaper={} lease_pid={pid} pgid={} recorded_start={} leader={:?} cmdline={:?}\n",
        std::process::id(),
        recorded.pgid,
        recorded.start,
        now.leader,
        cmdline
    );
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
    {
        let _ = file.write_all(line.as_bytes());
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
                let now = observe(&recorded);
                match verdict(&recorded, &now) {
                    Verdict::Keep => {
                        if now.leader == Leader::Unknown {
                            log_decision(dir, "keep-unknown", pid, &recorded, &now);
                        }
                        continue;
                    }
                    Verdict::Drop => {}
                    Verdict::Stop => {
                        log_decision(dir, "stop", pid, &recorded, &now);
                        stop_group(&recorded);
                        reaped.push(Reaped {
                            pid,
                            pgid: recorded.pgid,
                        });
                    }
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

/// True while the recorded group's leader is still provably the recorded one or gone.
fn still_recorded(recorded: &Recorded) -> bool {
    match sys::leader(recorded.pgid) {
        Leader::Present(start) => start == recorded.start,
        Leader::Absent => true,
        Leader::Unknown => false,
    }
}

fn stop_group(recorded: &Recorded) {
    const SIGTERM: i32 = 15;
    const SIGKILL: i32 = 9;
    let pgid = recorded.pgid;
    sys::signal_group(pgid, SIGTERM);
    let deadline = std::time::Instant::now() + REAP_GRACE;
    while group_alive(pgid) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if group_alive(pgid) && still_recorded(recorded) {
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
