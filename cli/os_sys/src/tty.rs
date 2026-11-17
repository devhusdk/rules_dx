//! The controlling terminal: its foreground group and size.

#![expect(
    unsafe_code,
    reason = "tcgetpgrp may return 0, which rustix's non-zero Pid cannot hold"
)]

use crate::signals::Blocked;
use std::io;
use std::os::fd::{AsFd, AsRawFd};

/// The process group in the foreground of `tty`; 0 when no group is in the foreground.
pub fn foreground_group(tty: impl AsFd) -> io::Result<i32> {
    // SAFETY: the descriptor is borrowed open for the call and tcgetpgrp has no memory preconditions.
    let group = unsafe { libc::tcgetpgrp(tty.as_fd().as_raw_fd()) };
    if group < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(group)
    }
}

/// Puts `group` in the foreground of `tty`. SIGTTOU stays blocked for the call,
/// so a background caller is not stopped for touching the terminal.
pub fn set_foreground_group(tty: impl AsFd, group: i32) -> io::Result<()> {
    let group = rustix::process::Pid::from_raw(group)
        .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
    let _blocked = Blocked::new(&[libc::SIGTTOU]);
    rustix::termios::tcsetpgrp(tty, group)?;
    Ok(())
}

/// The width of the terminal behind `fd` in columns; 0 when the terminal does not say.
pub fn columns(fd: impl AsFd) -> io::Result<u16> {
    Ok(rustix::termios::tcgetwinsize(fd)?.ws_col)
}
