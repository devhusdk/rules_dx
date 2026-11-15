//! Native process probes: liveness, the process table and command lines.

/// Whether `pid` names a running process.
#[expect(unsafe_code, reason = "kill with signal 0 has no safe std API")]
pub fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: kill with signal 0 only checks that the process exists.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// The start time of `pid` as an opaque string, or `None` when it cannot be read.
#[cfg(target_os = "linux")]
pub fn start_time(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19).map(str::to_owned)
}

/// The start time of `pid` as an opaque string, or `None` when it cannot be read.
#[cfg(target_os = "macos")]
pub fn start_time(pid: u32) -> Option<String> {
    let (secs, micros) = start_timeval(libc::pid_t::try_from(pid).ok()?)?;
    Some(format!("{secs}.{micros:06}"))
}

/// The kernel start time of `pid` as seconds and microseconds, read without allocating.
/// `p_starttime` is the first field of the `kinfo_proc` that `KERN_PROC_PID` returns.
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "sysctl KERN_PROC_PID has no safe std API")]
pub(crate) fn start_timeval(pid: libc::pid_t) -> Option<(i64, i64)> {
    let mut info = [0u64; 128];
    let mut len = std::mem::size_of_val(&info);
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    // SAFETY: `info` is writable for `len` bytes and `mib` names a read-only query.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            4,
            info.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len < std::mem::size_of::<libc::timeval>() {
        return None;
    }
    // SAFETY: the kernel wrote at least one `timeval` at the start of the aligned buffer.
    let start: libc::timeval = unsafe { std::ptr::read(info.as_ptr().cast()) };
    let micros = i64::from(start.tv_usec);
    (start.tv_sec != 0 || micros != 0).then_some((start.tv_sec, micros))
}

/// The start time of `pid` as an opaque string, or `None` when it cannot be read.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn start_time(pid: u32) -> Option<String> {
    let output = start_time_command(pid).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.split_whitespace().collect::<Vec<_>>().join("_");
    (!text.is_empty()).then_some(text)
}

/// The `ps` call behind `start_time`; the C locale keeps its text the same for every caller.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn start_time_command(pid: u32) -> std::process::Command {
    let mut command = std::process::Command::new("ps");
    command
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}

/// The id of the current boot, or `None` when the OS does not give one.
#[cfg(target_os = "linux")]
pub fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The id of the current boot, or `None` when the OS does not give one.
#[cfg(not(target_os = "linux"))]
pub fn boot_id() -> Option<String> {
    None
}
