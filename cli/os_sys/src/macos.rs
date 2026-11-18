//! macOS process table and memory queries through `sysctl` and libproc.

#![expect(
    unsafe_code,
    reason = "sysctl and proc_listallpids have no safe std or rustix API"
)]

use std::ffi::CStr;

/// Size of `struct kinfo_proc` on 64-bit macOS.
const KINFO_PROC_SIZE: usize = 648;

/// Offset of `kp_proc.p_stat` within `struct kinfo_proc` on 64-bit macOS.
const KINFO_PROC_P_STAT: usize = 36;

/// Reads the `sysctl` value named by `mib` into `buffer`; returns the bytes written.
fn sysctl_into(mib: &mut [libc::c_int], buffer: &mut [u8]) -> Option<usize> {
    let depth = libc::c_uint::try_from(mib.len()).ok()?;
    let mut len = buffer.len();
    // SAFETY: `buffer` is writable for `len` bytes and `mib` names a read-only query of `depth` levels.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            depth,
            buffer.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0).then_some(len)
}

/// The kernel start time of `pid` as seconds and microseconds, read without allocating.
/// `p_starttime` is the first field of the `kinfo_proc` that `KERN_PROC_PID` returns.
#[must_use]
pub fn start_timeval(pid: libc::pid_t) -> Option<(i64, i64)> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    let mut bytes = [0u8; KINFO_PROC_SIZE];
    let len = sysctl_into(&mut mib, &mut bytes)?;
    if len < 16 {
        return None;
    }
    // `struct timeval` on 64-bit macOS is an i64 `tv_sec` then an i32 `tv_usec` padded to 8 bytes.
    let secs = i64::from_ne_bytes(bytes.get(..8)?.try_into().ok()?);
    let micros = i64::from(i32::from_ne_bytes(bytes.get(8..12)?.try_into().ok()?));
    (secs != 0 || micros != 0).then_some((secs, micros))
}

/// Whether `pid` is a zombie: exited but not yet reaped by its parent.
#[must_use]
pub fn zombie(pid: u32) -> bool {
    let Ok(pid) = libc::c_int::try_from(pid) else {
        return false;
    };
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    let mut info = [0u8; KINFO_PROC_SIZE];
    sysctl_into(&mut mib, &mut info) == Some(KINFO_PROC_SIZE)
        && info
            .get(KINFO_PROC_P_STAT)
            .is_some_and(|&stat| u32::from(stat) == libc::SZOMB)
}

/// Every pid in the process table.
#[must_use]
pub fn all_pids() -> Vec<i32> {
    // SAFETY: a null buffer asks only for the current process count.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let Ok(count) = usize::try_from(count) else {
        return Vec::new();
    };
    let mut pids: Vec<libc::c_int> = vec![0; count + 64];
    let Ok(bytes) = libc::c_int::try_from(std::mem::size_of_val(pids.as_slice())) else {
        return Vec::new();
    };
    // SAFETY: `pids` is writable for `bytes` bytes.
    let listed = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    pids.truncate(usize::try_from(listed).unwrap_or(0));
    pids
}

/// The raw `KERN_PROCARGS2` buffer of `pid`.
#[must_use]
pub fn procargs(pid: i32) -> Option<Vec<u8>> {
    let mut argmax = [0u8; std::mem::size_of::<libc::c_int>()];
    let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
    sysctl_into(&mut mib, &mut argmax)?;
    let argmax = usize::try_from(libc::c_int::from_ne_bytes(argmax)).ok()?;
    let mut buffer = vec![0u8; argmax];
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let len = sysctl_into(&mut mib, &mut buffer)?;
    buffer.truncate(len);
    Some(buffer)
}

/// Reads the fixed-size `sysctl` value called `name`; `None` unless exactly `N` bytes come back.
fn sysctl_named<const N: usize>(name: &CStr) -> Option<[u8; N]> {
    let mut value = [0u8; N];
    let mut len = N;
    // SAFETY: `name` is NUL-terminated, `value` is writable for `len` bytes and no new value is set.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && len == N).then_some(value)
}

/// The `int` `sysctl` value called `name`, such as `kern.memorystatus_vm_pressure_level`.
#[must_use]
pub fn sysctl_i32(name: &CStr) -> Option<i32> {
    sysctl_named(name).map(i32::from_ne_bytes)
}

/// Physical memory reported by `hw.memsize`.
#[must_use]
pub fn memsize() -> Option<u64> {
    sysctl_named(c"hw.memsize").map(u64::from_ne_bytes)
}
