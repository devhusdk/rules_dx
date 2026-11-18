//! File system space.

use std::io;
use std::path::Path;

/// Bytes on the file system holding `dir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    /// What an unprivileged user may still write, the figure `df` reports.
    pub free: u64,
    pub total: u64,
}

/// The space of the file system holding `dir`, from `statvfs(3)`.
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
