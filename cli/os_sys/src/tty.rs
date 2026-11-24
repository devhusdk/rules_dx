#![expect(
    unsafe_code,
    reason = "tcgetpgrp may return 0, which rustix's non-zero Pid cannot hold"
)]

use crate::signals::Blocked;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

pub fn foreground_group(tty: impl AsFd) -> io::Result<i32> {
    // SAFETY: the descriptor is borrowed open for the call and tcgetpgrp has no memory preconditions.
    let group = unsafe { libc::tcgetpgrp(tty.as_fd().as_raw_fd()) };
    if group < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(group)
    }
}

pub fn set_foreground_group(tty: impl AsFd, group: i32) -> io::Result<()> {
    let group = rustix::process::Pid::from_raw(group)
        .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
    let _blocked = Blocked::new(&[libc::SIGTTOU]);
    rustix::termios::tcsetpgrp(tty, group)?;
    Ok(())
}

pub fn columns(fd: impl AsFd) -> io::Result<u16> {
    Ok(rustix::termios::tcgetwinsize(fd)?.ws_col)
}

pub fn open_pty() -> io::Result<(OwnedFd, CString)> {
    use rustix::pty::OpenptFlags;
    let master = rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    rustix::io::fcntl_setfd(&master, rustix::io::FdFlags::CLOEXEC)?;
    rustix::pty::grantpt(&master)?;
    rustix::pty::unlockpt(&master)?;
    let slave = rustix::pty::ptsname(&master, Vec::new())?;
    Ok((master, slave))
}
