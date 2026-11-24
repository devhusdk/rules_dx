#![cfg_attr(
    windows,
    expect(unsafe_code, reason = "GetDiskFreeSpaceExW has no safe std API")
)]

use std::io;
#[cfg(unix)]
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    pub free: u64,
    pub total: u64,
}

#[cfg(unix)]
pub fn space(dir: &Path) -> io::Result<Space> {
    let stat = rustix::fs::statvfs(dir)?;
    let overflow = || io::Error::other(format!("statvfs of {} overflows u64", dir.display()));
    Ok(Space {
        free: stat
            .f_bavail
            .checked_mul(stat.f_frsize)
            .ok_or_else(overflow)?,
        total: stat
            .f_blocks
            .checked_mul(stat.f_frsize)
            .ok_or_else(overflow)?,
    })
}

#[cfg(windows)]
pub fn space(dir: &Path) -> io::Result<Space> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut wide: Vec<u16> = dir.as_os_str().encode_wide().collect();
    if !matches!(wide.last(), Some(&c) if c == u16::from(b'\\') || c == u16::from(b'/')) {
        wide.push(u16::from(b'\\'));
    }
    wide.push(0);
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: wide is NUL-terminated and both outputs are writable u64s; the third may be null.
    let ok =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Space { free, total })
}

#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "rustix fcntl_getlk debug-asserts a non-negative l_pid, and Linux reports -1 for OFD locks"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the fcntl lock constants fit the short flock fields"
)]
pub fn write_locked(file: impl AsFd) -> io::Result<bool> {
    // SAFETY: a zeroed `flock` is a valid value; every field the call reads is set below.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock.l_start = 0;
    lock.l_len = 0;
    // SAFETY: the fd is borrowed open for the call and `lock` is a valid, exclusively borrowed `flock`.
    let status = unsafe { libc::fcntl(file.as_fd().as_raw_fd(), libc::F_GETLK, &raw mut lock) };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(lock.l_type != libc::F_UNLCK as libc::c_short)
}
