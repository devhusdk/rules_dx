//! File descriptor flags, pipes and readiness.

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::FdFlags;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::time::Duration;

/// A close-on-exec, non-blocking pipe: (read end, write end).
pub fn pipe_nonblocking() -> io::Result<(OwnedFd, OwnedFd)> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use rustix::pipe::PipeFlags;
        Ok(rustix::pipe::pipe_with(
            PipeFlags::CLOEXEC | PipeFlags::NONBLOCK,
        )?)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let (read, write) = rustix::pipe::pipe()?;
        for end in [&read, &write] {
            rustix::io::fcntl_setfd(end, FdFlags::CLOEXEC)?;
            let flags = rustix::fs::fcntl_getfl(end)?;
            rustix::fs::fcntl_setfl(end, flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        Ok((read, write))
    }
}

/// Clears close-on-exec on `fd` so an exec'd program inherits it; async-signal-safe.
pub fn set_inheritable(fd: impl AsFd) -> io::Result<()> {
    let flags = rustix::io::fcntl_getfd(&fd)?;
    rustix::io::fcntl_setfd(&fd, flags.difference(FdFlags::CLOEXEC))?;
    Ok(())
}

/// Whether `fd` became readable within `wait`.
pub fn wait_readable(fd: impl AsFd, wait: Duration) -> io::Result<bool> {
    let timeout =
        Timespec::try_from(wait).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    Ok(rustix::event::poll(&mut fds, Some(&timeout))? > 0)
}
