use std::fs::{File, OpenOptions};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const BYTES_PER_SLOT: u64 = 3 << 30;

pub const BYTES_PER_NESTED_BAZEL: u64 = 2 << 30;

pub const MAX_SLOTS: usize = 16;

#[must_use]
pub fn slot_count(memory: u64, reserved: u64) -> usize {
    usize::try_from(memory.saturating_sub(reserved) / BYTES_PER_SLOT)
        .unwrap_or(MAX_SLOTS)
        .clamp(1, MAX_SLOTS)
}

pub fn slots_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let cache = crate::user_cache_dir(env_get)?;
    Some(cache.join("dx").join("slots"))
}

#[must_use]
pub fn slot_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.lock"))
}

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
        return Err(error); // LCOV_EXCL_LINE - reason: a write-protected non-file slot cannot be built portably, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    let locked = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    match locked {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(_)) => Err(error), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[cfg(target_os = "linux")]
fn tightest(total: Option<u64>, limit: Option<u64>) -> Option<u64> {
    match (total, limit) {
        (Some(total), Some(limit)) => Some(total.min(limit)),
        (total, limit) => total.or(limit),
    }
}

#[cfg(target_os = "linux")]
#[must_use]
pub fn available_memory() -> Option<u64> {
    let total = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| parse_meminfo(&text));
    tightest(total, cgroup_limit())
}

#[cfg(target_os = "macos")]
pub fn available_memory() -> Option<u64> {
    dx_os_sys::macos::memsize()
}

#[cfg(windows)]
pub fn available_memory() -> Option<u64> {
    dx_os_sys::win32::total_physical_memory().ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn available_memory() -> Option<u64> {
    None
}

#[must_use]
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

#[must_use]
pub fn parse_cgroup_limit(text: &str) -> Option<u64> {
    text.trim().parse().ok().filter(|value| *value < 1 << 62)
}

#[cfg(target_os = "linux")]
fn cgroup_limit() -> Option<u64> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    cgroup_limit_in(&own, Path::new("/sys/fs/cgroup"))
}

#[must_use]
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

#[must_use]
pub fn reserve_range(memory: u64, servers: u64) -> Range<usize> {
    let reserved = servers.saturating_mul(BYTES_PER_NESTED_BAZEL);
    slot_count(memory, reserved)..slot_count(memory, 0)
}

#[derive(Debug, Default)]
pub struct Reserve {
    pub held: Vec<File>,
    pub busy: Vec<usize>,
}

#[must_use]
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
