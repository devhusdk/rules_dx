#![expect(
    unsafe_code,
    reason = "fork, sigaction and signal handler bodies have no safe API"
)]

use crate::STOP_SIGNALS;
use crate::errno::ErrnoGuard;
use crate::signals::{Blocked, set_of};
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::{AtomicI32, Ordering};

pub const JOB_STOPS: [i32; 3] = [libc::SIGTSTP, libc::SIGTTIN, libc::SIGTTOU];
pub const SETTLE: i32 = libc::SIGUSR1;
pub const SETTLED: u8 = 0;
const WATCHED: [i32; 4] = [STOP_SIGNALS[0], STOP_SIGNALS[1], STOP_SIGNALS[2], SETTLE];
const MAX_CLOSE: i32 = 65536;

static WATCHER_PARENT: AtomicI32 = AtomicI32::new(0);
static WATCHER_REPORT: AtomicI32 = AtomicI32::new(-1);

// LCOV_EXCL_START - reason: needs a controlling terminal in the foreground, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
pub fn spawn_watcher(report: OwnedFd) -> io::Result<i32> {
    let parent = rustix::process::getpid().as_raw_nonzero().get();
    WATCHER_PARENT.store(parent, Ordering::SeqCst);
    WATCHER_REPORT.store(report.as_raw_fd(), Ordering::SeqCst);
    let forked = {
        let _blocked = Blocked::new(&WATCHED);
        // SAFETY: fork takes no arguments; the child immediately enters `watch`.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            watch(parent, report.as_raw_fd());
        }
        if pid < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(pid)
        }
    };
    drop(report);
    forked
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(any(target_os = "linux", target_os = "android"))]
fn user_sent(code: i32) -> bool {
    code <= 0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn user_sent(_code: i32) -> bool {
    true
}

fn from_parent(info: Option<&libc::siginfo_t>) -> bool {
    let Some(info) = info else {
        return false;
    };
    // SAFETY: info is a siginfo_t the kernel filled for a delivered signal, so si_pid is set.
    let sender = unsafe { info.si_pid() };
    sender == WATCHER_PARENT.load(Ordering::Relaxed) && user_sent(info.si_code)
}

extern "C" fn report_signal(signo: libc::c_int, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    let _errno = ErrnoGuard::save();
    // SAFETY: info is null or the siginfo_t the kernel passed to this handler.
    if !from_parent(unsafe { info.as_ref() }) {
        report(signo);
    }
}

extern "C" fn report_settled(_: libc::c_int, _: *mut libc::siginfo_t, _: *mut libc::c_void) {
    let _errno = ErrnoGuard::save();
    for signo in crate::signals::pending(&STOP_SIGNALS) {
        report(signo);
    }
    report(libc::c_int::from(SETTLED));
}

fn report(signo: libc::c_int) {
    let byte = u8::try_from(signo).unwrap_or(SETTLED);
    // SAFETY: byte is a valid one-byte buffer; a closed or invalid descriptor only fails the write.
    unsafe {
        libc::write(
            WATCHER_REPORT.load(Ordering::Relaxed),
            std::ptr::addr_of!(byte).cast(),
            1,
        )
    };
}

// LCOV_EXCL_START - reason: runs only in the forked watcher, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
fn close_others(keep: i32) {
    #[cfg(target_os = "linux")]
    {
        let all = libc::c_uint::MAX;
        let keep_u = libc::c_uint::try_from(keep).unwrap_or(0);
        let ranges: [(libc::c_uint, libc::c_uint); 2] = if keep_u >= 3 {
            [(3, keep_u - 1), (keep_u.saturating_add(1), all)]
        } else {
            [(3, all), (1, 0)]
        };
        let mut closed = true;
        for (first, last) in ranges {
            // SAFETY: close_range takes plain integers and only closes descriptors.
            if first <= last && unsafe { libc::syscall(libc::SYS_close_range, first, last, 0) } != 0
            {
                closed = false;
            }
        }
        if closed {
            return;
        }
    }
    let top = rustix::process::getrlimit(rustix::process::Resource::Nofile)
        .current
        .and_then(|current| i32::try_from(current).ok())
        .map_or(MAX_CLOSE, |current| current.min(MAX_CLOSE));
    for fd in (3..top).filter(|&fd| fd != keep) {
        // SAFETY: the forked watcher uses no descriptor other than the kept report pipe.
        unsafe { libc::close(fd) };
    }
}

fn quit() -> ! {
    // SAFETY: _exit ends this process at once and has no preconditions.
    unsafe { libc::_exit(0) }
}

fn watch(parent: i32, report: i32) -> ! {
    match rustix::process::setpgid(None, None) {
        Ok(()) | Err(_) => {}
    }
    close_others(report);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    drop(crate::process::die_with_parent(libc::SIGKILL));
    if rustix::process::getppid().map(|pid| pid.as_raw_nonzero().get()) != Some(parent) {
        quit();
    }
    for signo in JOB_STOPS {
        // SAFETY: SIG_IGN installs no handler code.
        unsafe { libc::signal(signo, libc::SIG_IGN) };
    }
    let unblock = set_of(&WATCHED);
    for signo in WATCHED {
        // SAFETY: sigaction is plain C data for which all-zero is valid.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = if signo == SETTLE {
            report_settled as *const () as libc::sighandler_t
        } else {
            report_signal as *const () as libc::sighandler_t
        };
        action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
        action.sa_mask = unblock;
        // SAFETY: action names an SA_SIGINFO handler that only calls async-signal-safe functions.
        unsafe { libc::sigaction(signo, &raw const action, std::ptr::null_mut()) };
    }
    // SAFETY: unblock is a valid, initialized sigset_t and a null old-set pointer is permitted.
    unsafe { libc::pthread_sigmask(libc::SIG_UNBLOCK, &raw const unblock, std::ptr::null_mut()) };
    loop {
        if rustix::process::getppid().map(|pid| pid.as_raw_nonzero().get()) != Some(parent) {
            quit();
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
#[path = "job_tests.rs"]
mod job_tests;
