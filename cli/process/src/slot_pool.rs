//! The machine-wide build slot pool: slot files under the user cache, their sizing, and the range of
//! indices kept back for nested Bazel test servers.

use std::fs::{File, OpenOptions};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Memory one Bazel server plus its build is planned to use.
pub const BYTES_PER_SLOT: u64 = 3 << 30;

/// Memory one nested Bazel test server is planned to use.
pub const BYTES_PER_NESTED_BAZEL: u64 = 2 << 30;

/// Most slots dx ever offers.
pub const MAX_SLOTS: usize = 16;

/// Slots for a machine with `memory` bytes, `reserved` of them kept aside.
pub fn slot_count(memory: u64, reserved: u64) -> usize {
    usize::try_from(memory.saturating_sub(reserved) / BYTES_PER_SLOT)
        .unwrap_or(MAX_SLOTS)
        .clamp(1, MAX_SLOTS)
}

/// The slot directory under the user cache.
pub fn slots_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let cache = crate::user_cache_dir(env_get)?;
    Some(cache.join("dx").join("slots"))
}

pub fn slot_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.lock"))
}

/// Locks a slot file shared or exclusive, falling back to a read-only open when it cannot be written.
pub fn lock_slot(path: &Path, shared: bool) -> io::Result<Option<File>> {
    let locked = if shared {
        dx_atomic_fs::lease::try_lock_file_shared(path, true)
    } else {
        dx_atomic_fs::lease::try_lock_file(path, true)
    };
    let error = match locked {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => error,
        other => return other,
    };
    let Ok(file) = OpenOptions::new().read(true).open(path) else {
        return Err(error);
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err(error);
    }
    let locked = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    match locked {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(_)) => Err(error),
    }
}

/// Total memory visible to this process, honoring a cgroup limit.
pub fn available_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let total = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| parse_meminfo(&text));
        let limit = cgroup_limit();
        return match (total, limit) {
            (Some(total), Some(limit)) => Some(total.min(limit)),
            (total, limit) => total.or(limit),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let mut value: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        let name = c"hw.memsize";
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&mut value as *mut u64).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        return (status == 0).then_some(value);
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatus {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        }
        let mut status = MemoryStatus {
            length: std::mem::size_of::<MemoryStatus>() as u32,
            load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page: 0,
            avail_page: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended: 0,
        };
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        return (ok != 0).then_some(status.total_phys);
    }
    #[allow(unreachable_code)]
    None
}

/// Reads `MemTotal` from `/proc/meminfo` text, in bytes.
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

/// Reads a cgroup memory limit; `max` or the v1 unlimited value means no limit.
pub fn parse_cgroup_limit(text: &str) -> Option<u64> {
    text.trim().parse().ok().filter(|value| *value < 1 << 62)
}

#[cfg(target_os = "linux")]
fn cgroup_limit() -> Option<u64> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    cgroup_limit_in(&own, Path::new("/sys/fs/cgroup"))
}

/// Reads the memory limit of the cgroup named in `own` under `root`.
pub fn cgroup_limit_in(own: &str, root: &Path) -> Option<u64> {
    let v2 = own
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or("/");
    let v1 = own.lines().find_map(|line| {
        let mut parts = line.splitn(3, ':');
        parts.next()?;
        let controllers = parts.next()?;
        let path = parts.next()?;
        controllers
            .split(',')
            .any(|c| c == "memory")
            .then_some(path)
    });
    let v2_limit = walk_limit(root, v2, "memory.max");
    let v1_limit =
        v1.and_then(|path| walk_limit(&root.join("memory"), path, "memory.limit_in_bytes"));
    match (v2_limit, v1_limit) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn walk_limit(base: &Path, relative: &str, file: &str) -> Option<u64> {
    let mut dir = base.join(relative.trim_start_matches('/'));
    let mut limit: Option<u64> = None;
    loop {
        let found = std::fs::read_to_string(dir.join(file))
            .ok()
            .and_then(|text| parse_cgroup_limit(&text));
        limit = match (limit, found) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if dir == base || !dir.pop() || !dir.starts_with(base) {
            return limit;
        }
    }
}

/// The slot indices kept for `servers` nested Bazel test servers on a machine with `memory` bytes.
///
/// Empty when `servers` is 0. dx gates leave these indices to nested test harnesses.
pub fn reserve_range(memory: u64, servers: u64) -> Range<usize> {
    let reserved = servers.saturating_mul(BYTES_PER_NESTED_BAZEL);
    slot_count(memory, reserved)..slot_count(memory, 0)
}

/// What [`hold_reserve`] took.
#[derive(Debug, Default)]
pub struct Reserve {
    /// The locked slot files; dropping them releases the locks.
    pub held: Vec<File>,
    /// Indices another process holds exclusively, or that could not be locked.
    pub busy: Vec<usize>,
}

/// Takes shared locks on the `range` slot files in `dir` without waiting.
///
/// Shared locks coexist with other reserve holders and with dx runs inside a gated run, and block
/// consumer dx runs from taking those indices exclusively.
pub fn hold_reserve(dir: &Path, range: Range<usize>) -> Reserve {
    let mut reserve = Reserve::default();
    if range.is_empty() || std::fs::create_dir_all(dir).is_err() {
        reserve.busy.extend(range);
        return reserve;
    }
    for index in range {
        match lock_slot(&slot_path(dir, index), true) {
            Ok(Some(file)) => reserve.held.push(file),
            _ => reserve.busy.push(index),
        }
    }
    reserve
}

#[cfg(test)]
#[path = "slot_pool_tests.rs"]
mod slot_pool_tests;
