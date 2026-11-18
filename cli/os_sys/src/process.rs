//! Process and process-group ids and signals.
//!
//! Group and session ids come from libc, not rustix: inside a PID namespace they
//! can be 0 (the leader lives outside), which rustix's non-zero `Pid` cannot hold.

#![expect(
    unsafe_code,
    reason = "getpgrp, getpgid and getsid may return 0, which rustix cannot represent"
)]

use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, WaitOptions};
use std::io;

pub use libc::{SIGCONT, SIGHUP, SIGINT, SIGKILL, SIGTERM, SIGTSTP, SIGTTIN, SIGTTOU, SIGUSR1};

/// What a signal-0 probe learned about a process or group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// It exists and this process may signal it.
    Reachable,
    /// It exists but belongs to someone this process may not signal.
    NotPermitted,
    /// Nothing has that id.
    Missing,
    /// The probe failed for another reason.
    Unknown,
}

// Both errors are built without allocating, so signal handlers can call these paths.
fn pid(raw: i32) -> io::Result<Pid> {
    Pid::from_raw(raw).ok_or(io::Error::from(io::ErrorKind::InvalidInput))
}

fn signal_of(signo: i32) -> io::Result<Signal> {
    Signal::from_named_raw(signo).ok_or(io::Error::from(io::ErrorKind::InvalidInput))
}

/// Sends `signo` like `kill(2)`: a positive `target` is a process, a negative one a group.
/// Async-signal-safe.
pub fn signal(target: i32, signo: i32) -> io::Result<()> {
    let sig = signal_of(signo)?;
    if target < 0 {
        rustix::process::kill_process_group(pid(target.saturating_neg())?, sig)?;
    } else {
        rustix::process::kill_process(pid(target)?, sig)?;
    }
    Ok(())
}

/// Probes `target` with signal 0: a positive `target` is a process, a negative one a group.
#[must_use]
pub fn probe(target: i32) -> Presence {
    let probed = if target < 0 {
        Pid::from_raw(target.saturating_neg()).map(rustix::process::test_kill_process_group)
    } else {
        Pid::from_raw(target).map(rustix::process::test_kill_process)
    };
    match probed {
        Some(Ok(())) => Presence::Reachable,
        Some(Err(rustix::io::Errno::PERM)) => Presence::NotPermitted,
        Some(Err(rustix::io::Errno::SRCH)) => Presence::Missing,
        Some(Err(_)) | None => Presence::Unknown,
    }
}

/// `-1` from a libc id call means failure, with the reason in `errno`.
fn id_result(id: libc::pid_t) -> io::Result<i32> {
    if id < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(id)
    }
}

/// The calling process's group id; 0 when the group leader is outside this PID namespace.
#[must_use]
pub fn own_group() -> i32 {
    // SAFETY: getpgrp has no preconditions and cannot fail.
    unsafe { libc::getpgrp() }
}

/// The calling process's session id; 0 when the session leader is outside this PID namespace.
pub fn own_session() -> io::Result<i32> {
    // SAFETY: getsid takes a plain pid; 0 names the calling process.
    id_result(unsafe { libc::getsid(0) })
}

/// The group id of `target`; 0 when the group leader is outside this PID namespace.
pub fn group_of(target: i32) -> io::Result<i32> {
    let target = pid(target)?.as_raw_nonzero().get();
    // SAFETY: getpgid takes a plain pid and has no memory preconditions.
    id_result(unsafe { libc::getpgid(target) })
}

/// Moves `target` into the group `group`, like `setpgid(2)`.
pub fn set_group(target: i32, group: i32) -> io::Result<()> {
    rustix::process::setpgid(Some(pid(target)?), Some(pid(group)?))?;
    Ok(())
}

/// The job-control signal that stopped `child`, if it is stopped now; never blocks.
pub fn stopped_by(child: i32) -> io::Result<Option<i32>> {
    let status = rustix::process::waitid(
        WaitId::Pid(pid(child)?),
        WaitIdOptions::STOPPED | WaitIdOptions::NOHANG,
    )?;
    Ok(status.and_then(|status| status.stopping_signal()))
}

/// Waits for `child` to exit and reaps it.
pub fn reap(child: i32) -> io::Result<()> {
    rustix::process::waitpid(Some(pid(child)?), WaitOptions::empty())?;
    Ok(())
}

/// Stops the calling process with SIGTSTP, as a shell expects of a stopped job.
pub fn stop_self() -> io::Result<()> {
    rustix::process::kill_process(rustix::process::getpid(), Signal::TSTP)?;
    Ok(())
}

/// Asks the kernel to send `signo` to the calling process when its parent dies; async-signal-safe.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn die_with_parent(signo: i32) -> io::Result<()> {
    rustix::process::set_parent_process_death_signal(Some(signal_of(signo)?))?;
    Ok(())
}

/// The start time field of a `/proc/<pid>/stat` text; never allocates.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[must_use]
pub fn stat_start(stat: &[u8]) -> Option<&[u8]> {
    let close = stat.iter().rposition(|&byte| byte == b')')?;
    stat.get(close + 1..)?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .nth(19)
}

/// The start time of `pid` as an opaque string, or `None` when it cannot be read.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[must_use]
pub fn start_time(pid: u32) -> Option<String> {
    let stat = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
    stat_start(&stat)
        .and_then(|start| std::str::from_utf8(start).ok())
        .map(str::to_owned)
}

/// The start time of `pid` as an opaque string, or `None` when it cannot be read.
#[cfg(target_os = "macos")]
#[must_use]
pub fn start_time(pid: u32) -> Option<String> {
    let (secs, micros) = crate::macos::start_timeval(i32::try_from(pid).ok()?)?;
    Some(format!("{secs}.{micros:06}"))
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod process_tests;
