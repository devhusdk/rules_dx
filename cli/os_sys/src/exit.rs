#![cfg_attr(
    target_os = "macos",
    expect(unsafe_code, reason = "rustix exposes kevent only as an unsafe fn")
)]

use std::io;
use std::time::{Duration, Instant};

pub fn wait_gone(pid: u32, limit: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + limit;
    wait_gone_until(pid, deadline)
}

#[cfg(target_os = "linux")]
fn wait_gone_until(pid: u32, deadline: Instant) -> io::Result<bool> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::io::Errno;
    use rustix::process::{Pid, PidfdFlags, pidfd_open};
    let raw = i32::try_from(pid).map_err(io::Error::other)?;
    let target = Pid::from_raw(raw).ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
    let fd = match pidfd_open(target, PidfdFlags::empty()) {
        Err(Errno::SRCH) => return Ok(true),
        opened => opened?,
    };
    loop {
        let left = Timespec::try_from(deadline.saturating_duration_since(Instant::now()))
            .map_err(io::Error::other)?;
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        match poll(&mut fds, Some(&left)) {
            Err(Errno::INTR) => {}
            Ok(ready) => return Ok(ready > 0),
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(target_os = "macos")]
fn wait_gone_until(pid: u32, deadline: Instant) -> io::Result<bool> {
    use rustix::buffer::spare_capacity;
    use rustix::event::kqueue::{Event, EventFilter, EventFlags, ProcessEvents, kevent, kqueue};
    use rustix::io::Errno;
    use rustix::process::Pid;
    let raw = i32::try_from(pid).map_err(io::Error::other)?;
    let target = Pid::from_raw(raw).ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
    let queue = kqueue()?;
    let exit = Event::new(
        EventFilter::Proc {
            pid: target,
            flags: ProcessEvents::EXIT,
        },
        EventFlags::ADD | EventFlags::ONESHOT,
        std::ptr::null_mut(),
    );
    let mut none: Vec<Event> = Vec::new();
    // SAFETY: the change list holds one process filter whose udata is null and never read.
    match unsafe { kevent(&queue, &[exit], &mut none, Some(Duration::ZERO)) } {
        Err(Errno::SRCH) => return Ok(true),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let mut fired: Vec<Event> = Vec::with_capacity(1);
        // SAFETY: the event list has room for one event and the queue holds only the exit filter.
        match unsafe { kevent(&queue, &[], spare_capacity(&mut fired), Some(left)) } {
            Err(Errno::INTR) => {}
            Ok(count) => return Ok(count > 0),
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(windows)]
fn wait_gone_until(pid: u32, deadline: Instant) -> io::Result<bool> {
    use std::os::windows::io::AsHandle;
    const ERROR_INVALID_PARAMETER: i32 = 87;
    let process = match crate::win32::open(pid, crate::win32::Access::Wait) {
        Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER) => return Ok(true),
        opened => opened?,
    };
    crate::win32::wait(
        process.as_handle(),
        Some(deadline.saturating_duration_since(Instant::now())),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn wait_gone_until(_pid: u32, _deadline: Instant) -> io::Result<bool> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}
