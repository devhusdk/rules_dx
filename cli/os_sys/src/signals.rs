//! The calling thread's signal mask.

#![expect(unsafe_code, reason = "signal masks have no safe std or rustix API")]

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
