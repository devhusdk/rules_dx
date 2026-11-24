#[must_use]
pub fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    matches!(
        dx_os_sys::process::probe(pid),
        dx_os_sys::process::Presence::Reachable | dx_os_sys::process::Presence::NotPermitted
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn start_time(pid: u32) -> Option<String> {
    dx_os_sys::process::start_time(pid)
}

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

#[cfg(target_os = "linux")]
#[must_use]
pub fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

#[cfg(not(target_os = "linux"))]
pub fn boot_id() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
#[must_use]
pub fn zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
    })
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn zombie(pid: u32) -> bool {
    dx_os_sys::macos::zombie(pid)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[must_use]
pub fn zombie(_pid: u32) -> bool {
    false
}

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

#[cfg(target_os = "macos")]
#[must_use]
pub fn command_line_table() -> Vec<(u32, String)> {
    dx_os_sys::macos::all_pids()
        .into_iter()
        .filter_map(|pid| {
            let args = parse_procargs(&dx_os_sys::macos::procargs(pid)?)?;
            Some((u32::try_from(pid).ok()?, args))
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[must_use]
pub fn command_line_table() -> Vec<(u32, String)> {
    Vec::new()
}

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
