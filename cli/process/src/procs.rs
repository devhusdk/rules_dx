//! Native process probes: liveness, the process table and command lines.

/// Whether `pid` names a running process.
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
#[cfg(not(target_os = "linux"))]
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
#[cfg(not(target_os = "linux"))]
fn start_time_command(pid: u32) -> std::process::Command {
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

#[cfg(test)]
#[path = "procs_tests.rs"]
mod procs_tests;
