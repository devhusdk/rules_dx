use dx_os_sys::nt;
use dx_os_sys::win32::{self, Access};
use std::os::windows::io::AsHandle;

const ERROR_ACCESS_DENIED: i32 = 5;

pub fn pid_alive(pid: u32) -> bool {
    match win32::open(pid, Access::Wait) {
        Ok(process) => win32::is_running(process.as_handle()).unwrap_or(false),
        Err(err) => err.raw_os_error() == Some(ERROR_ACCESS_DENIED),
    }
}

pub fn start_time(pid: u32) -> Option<String> {
    let process = win32::open(pid, Access::Query).ok()?;
    win32::creation_time(process.as_handle())
        .ok()
        .map(|ticks| ticks.to_string())
}

pub fn boot_id() -> Option<String> {
    None
}

pub fn process_table() -> Vec<(u32, u32, String)> {
    win32::processes()
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.pid, entry.parent, entry.exe))
        .collect()
}

pub fn command_lines() -> Vec<ProcessInfo> {
    let mut infos = Vec::new();
    for entry in win32::processes().unwrap_or_default() {
        let Ok(process) = win32::open(entry.pid, Access::Query) else {
            continue;
        };
        let Ok(command_line) = nt::command_line(process.as_handle()) else {
            continue; // LCOV_EXCL_LINE - reason: a process that exits while it is queried, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        };
        infos.push(ProcessInfo {
            pid: entry.pid,
            rss_bytes: win32::working_set_bytes(process.as_handle()).unwrap_or(0),
            command_line,
        });
    }
    infos
}

/// A running process with its command line and resident memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub rss_bytes: u64,
    pub command_line: String,
}
