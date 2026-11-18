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

/// The calling thread's `errno` location.
#[expect(unsafe_code, reason = "the errno location is only exposed by libc")]
#[must_use]
pub fn slot() -> *mut libc::c_int {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    // SAFETY: __errno_location has no preconditions.
    unsafe {
        libc::__errno_location()
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    // SAFETY: __error has no preconditions.
    unsafe {
        libc::__error()
    }
}
