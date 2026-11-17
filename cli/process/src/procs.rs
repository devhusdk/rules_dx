//! Native process probes: liveness, the process table and command lines.

/// Whether `pid` names a running process.
#[expect(unsafe_code, reason = "kill with signal 0 has no safe std API")]
#[must_use]
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
#[must_use]
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

/// Whether `pid` is a zombie: exited but not yet reaped by its parent.
#[cfg(target_os = "linux")]
#[must_use]
pub fn zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
    })
}

/// Size of `struct kinfo_proc` on 64-bit macOS.
#[cfg(target_os = "macos")]
const KINFO_PROC_SIZE: usize = 648;

/// Offset of `kp_proc.p_stat` within `struct kinfo_proc` on 64-bit macOS.
#[cfg(target_os = "macos")]
const KINFO_PROC_P_STAT: usize = 36;

/// Whether `pid` is a zombie: exited but not yet reaped by its parent.
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "sysctl KERN_PROC_PID has no safe std API")]
#[must_use]
pub fn zombie(pid: u32) -> bool {
    let Ok(pid) = libc::c_int::try_from(pid) else {
        return false;
    };
    let mut buffer = [0u64; KINFO_PROC_SIZE / 8];
    let mut len = std::mem::size_of_val(&buffer);
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    // SAFETY: `buffer` is writable for `len` bytes and `mib` names a read-only query.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            4,
            buffer.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len != KINFO_PROC_SIZE {
        return false;
    }
    buffer
        .iter()
        .flat_map(|word| word.to_ne_bytes())
        .nth(KINFO_PROC_P_STAT)
        .is_some_and(|stat| u32::from(stat) == libc::SZOMB)
}

/// Whether `pid` is a zombie: exited but not yet reaped by its parent.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[must_use]
pub fn zombie(_pid: u32) -> bool {
    false
}

/// Every readable process as `(pid, arguments joined by NUL)`.
#[cfg(target_os = "linux")]
#[must_use]
pub fn command_line_table() -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .filter_map(|pid| {
            let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
            Some((pid, String::from_utf8_lossy(&cmdline).into_owned()))
        })
        .collect()
}

/// Every readable process as `(pid, arguments joined by NUL)`.
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "proc_listallpids has no safe std API")]
#[must_use]
pub fn command_line_table() -> Vec<(u32, String)> {
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
    pids.into_iter()
        .filter_map(|pid| {
            let args = procargs(pid)?;
            Some((u32::try_from(pid).ok()?, args))
        })
        .collect()
}

/// Every readable process as `(pid, arguments joined by NUL)`.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[must_use]
pub fn command_line_table() -> Vec<(u32, String)> {
    Vec::new()
}

/// The arguments of `pid` from `KERN_PROCARGS2`, joined by NUL.
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "sysctl KERN_PROCARGS2 has no safe std API")]
fn procargs(pid: libc::c_int) -> Option<String> {
    let mut argmax: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&argmax);
    let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
    // SAFETY: `argmax` is writable for `len` bytes and `mib` names a read-only query.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            (&raw mut argmax).cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    let mut buffer = vec![0u8; usize::try_from(argmax).ok()?];
    let mut len = buffer.len();
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    // SAFETY: `buffer` is writable for `len` bytes and `mib` names a read-only query.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    buffer.truncate(len);
    parse_procargs(&buffer)
}

/// Parses a `KERN_PROCARGS2` buffer: `argc`, the executable path, NUL padding, then `argc` arguments.
#[must_use]
pub fn parse_procargs(buffer: &[u8]) -> Option<String> {
    let (argc, rest) = buffer.split_first_chunk::<4>()?;
    let argc = usize::try_from(i32::from_ne_bytes(*argc)).ok()?;
    let after_path = rest.iter().position(|byte| *byte == 0)?;
    let rest = rest.get(after_path..)?;
    let start = rest.iter().position(|byte| *byte != 0)?;
    let args = rest
        .get(start..)?
        .split(|byte| *byte == 0)
        .take(argc)
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>();
    (args.len() == argc).then(|| args.join("\0"))
}
