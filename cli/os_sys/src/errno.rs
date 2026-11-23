#[must_use = "errno is restored when the guard drops"]
pub struct ErrnoGuard(::errno::Errno);

impl ErrnoGuard {
    pub fn save() -> ErrnoGuard {
        ErrnoGuard(::errno::errno())
    }
}

impl Drop for ErrnoGuard {
    fn drop(&mut self) {
        ::errno::set_errno(self.0);
    }
}

#[must_use]
pub fn get() -> i32 {
    ::errno::errno().0
}

pub fn set(code: i32) {
    ::errno::set_errno(::errno::Errno(code));
}
