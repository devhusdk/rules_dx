//! The calling thread's signal mask.

#![expect(unsafe_code, reason = "signal masks have no safe std or rustix API")]

use std::ffi::{CString, c_char};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicPtr, Ordering};

static UNLINK_ON_STOP: AtomicPtr<c_char> = AtomicPtr::new(std::ptr::null_mut());

/// Blocks signals in the calling thread until dropped.
pub struct Blocked {
    previous: libc::sigset_t,
}

impl Blocked {
    /// Blocks `signals` in the calling thread; async-signal-safe.
    #[must_use = "the signals are unblocked again when the guard drops"]
    pub fn new(signals: &[i32]) -> Blocked {
        let set = set_of(signals);
        let mut previous = empty_set();
        // SAFETY: set and previous are valid, initialized sigset_t values.
        unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &raw const set, &raw mut previous) };
        Blocked { previous }
    }
}

impl Drop for Blocked {
    fn drop(&mut self) {
        // SAFETY: previous was saved by pthread_sigmask; a null old-set pointer is permitted.
        unsafe {
            libc::pthread_sigmask(
                libc::SIG_SETMASK,
                &raw const self.previous,
                std::ptr::null_mut(),
            )
        };
    }
}

/// Lets the calling thread write to the terminal while the process is in the background.
pub fn allow_background_tty_writes() {
    let set = set_of(&[libc::SIGTTOU]);
    // SAFETY: set is a valid, initialized sigset_t and a null old-set pointer is permitted.
    unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &raw const set, std::ptr::null_mut()) };
}

/// Whether `signo` is blocked in the calling thread.
#[must_use]
pub fn blocked(signo: i32) -> bool {
    let mut current = empty_set();
    // SAFETY: current is a valid sigset_t out-pointer and a null new set only reads the mask.
    unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), &raw mut current) };
    member(&current, signo)
}

/// Which of `signals` are pending for the calling thread or process; async-signal-safe.
pub fn pending(signals: &[i32]) -> impl Iterator<Item = i32> {
    let mut set = empty_set();
    // SAFETY: set is a valid sigset_t out-pointer.
    unsafe { libc::sigpending(&raw mut set) };
    signals
        .iter()
        .copied()
        .filter(move |&signo| member(&set, signo))
}

// LCOV_EXCL_START - reason: replaces the test process's own stop handlers, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
/// Installs `handler` for SIGINT, SIGTERM and SIGHUP with `SA_RESTART`; an inherited ignored SIGHUP stays ignored.
///
/// # Safety
///
/// `handler` must call only async-signal-safe functions.
pub unsafe fn handle_stops(handler: extern "C" fn(i32)) -> io::Result<()> {
    // SAFETY: sigaction is plain C data for which all-zero is valid.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = handler as *const () as libc::sighandler_t;
    action.sa_flags = libc::SA_RESTART;
    action.sa_mask = empty_set();
    for signo in crate::STOP_SIGNALS {
        // SAFETY: sigaction is plain C data for which all-zero is valid.
        let mut inherited: libc::sigaction = unsafe { std::mem::zeroed() };
        // SAFETY: action and inherited are valid sigaction values; the caller vouches for the handler.
        check(unsafe { libc::sigaction(signo, &raw const action, &raw mut inherited) })?;
        if signo == libc::SIGHUP && inherited.sa_sigaction == libc::SIG_IGN {
            // SAFETY: inherited was filled by sigaction and a null old-action pointer is permitted.
            check(unsafe { libc::sigaction(signo, &raw const inherited, std::ptr::null_mut()) })?;
        }
    }
    Ok(())
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

/// Restores the default disposition of `signo` and raises it in the calling process.
pub fn raise_default(signo: i32) -> io::Result<()> {
    // SAFETY: SIG_DFL installs no handler code.
    if unsafe { libc::signal(signo, libc::SIG_DFL) } == libc::SIG_ERR {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raise takes a plain signal number.
    check(unsafe { libc::raise(signo) })
}

/// Ignores `signo` in the calling process; async-signal-safe.
pub fn ignore(signo: i32) -> io::Result<()> {
    // SAFETY: SIG_IGN installs no handler code.
    if unsafe { libc::signal(signo, libc::SIG_IGN) } == libc::SIG_ERR {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

extern "C" fn unlink_and_exit(signo: i32) {
    let path = UNLINK_ON_STOP.load(Ordering::SeqCst);
    if !path.is_null() {
        // SAFETY: path is a leaked NUL-terminated CString and unlink is async-signal-safe.
        unsafe { libc::unlink(path) };
    }
    // SAFETY: _exit is async-signal-safe.
    unsafe { libc::_exit(128 + signo) }
}

/// On SIGINT or SIGTERM, removes `path` and exits with 128 plus the signal number.
/// A later call replaces the path; [`keep_on_stop`] clears it.
pub fn unlink_on_stop(path: &Path) -> io::Result<()> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} contains a NUL byte at offset {}",
                path.display(),
                error.nul_position()
            ),
        )
    })?;
    // Any earlier path is leaked: the handler may be reading it right now.
    UNLINK_ON_STOP.store(path.into_raw(), Ordering::SeqCst);
    // SAFETY: sigaction is plain C data for which all-zero is valid.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = unlink_and_exit as *const () as libc::sighandler_t;
    action.sa_mask = empty_set();
    for signo in [libc::SIGINT, libc::SIGTERM] {
        // SAFETY: action is a valid sigaction whose handler calls only unlink and _exit.
        check(unsafe { libc::sigaction(signo, &raw const action, std::ptr::null_mut()) })?;
    }
    Ok(())
}

/// Holds SIGINT and SIGTERM in the calling thread until dropped; a held signal then lands.
#[must_use = "the stop signals are released again when the guard drops"]
pub fn hold_stop() -> Blocked {
    Blocked::new(&[libc::SIGINT, libc::SIGTERM])
}

/// Stops [`unlink_on_stop`] from removing its path; the signals still exit the process.
pub fn keep_on_stop() {
    UNLINK_ON_STOP.store(std::ptr::null_mut(), Ordering::SeqCst);
}

fn check(status: libc::c_int) -> io::Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) fn empty_set() -> libc::sigset_t {
    // SAFETY: sigset_t is plain C data for which all-zero is valid.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: set is a valid sigset_t.
    unsafe { libc::sigemptyset(&raw mut set) };
    set
}

pub(crate) fn set_of(signals: &[i32]) -> libc::sigset_t {
    let mut set = empty_set();
    for &signo in signals {
        // SAFETY: set is a valid, initialized sigset_t.
        unsafe { libc::sigaddset(&raw mut set, signo) };
    }
    set
}

fn member(set: &libc::sigset_t, signo: i32) -> bool {
    // SAFETY: set is a valid, initialized sigset_t.
    unsafe { libc::sigismember(set, signo) == 1 }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod signals_tests;
