//! The calling thread's `errno`, saved and restored around signal-handler work.

/// Restores the thread's `errno` when dropped, so a signal handler leaves it as it found it.
#[must_use = "errno is restored when the guard drops"]
pub struct ErrnoGuard(::errno::Errno);

impl ErrnoGuard {
    /// Saves the current `errno`; async-signal-safe.
    pub fn save() -> ErrnoGuard {
        ErrnoGuard(::errno::errno())
    }
}

impl Drop for ErrnoGuard {
    fn drop(&mut self) {
        ::errno::set_errno(self.0);
    }
}

/// The calling thread's `errno`.
#[must_use]
pub fn get() -> i32 {
    ::errno::errno().0
}

/// Sets the calling thread's `errno`.
pub fn set(code: i32) {
    ::errno::set_errno(::errno::Errno(code));
}
