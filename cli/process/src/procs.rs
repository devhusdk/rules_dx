//! Native process probes: liveness, the process table and command lines.

/// Whether `pid` names a running process.
#[cfg(unix)]
pub fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: kill with signal 0 only checks that the process exists.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// A running process with its command line and resident memory.
#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub rss_bytes: u64,
    pub command_line: String,
}

#[cfg(windows)]
mod win {
    use super::ProcessInfo;
    use std::ffi::c_void;
    use std::sync::OnceLock;

    type Handle = *mut c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const WAIT_TIMEOUT: u32 = 0x102;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
    const STATUS_BUFFER_OVERFLOW: i32 = 0x8000_0005_u32 as i32;
    const STATUS_BUFFER_TOO_SMALL: i32 = 0xC000_0023_u32 as i32;
    const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        exe: [u16; 260],
    }

    #[repr(C)]
    struct MemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    type NtQuery = unsafe extern "system" fn(Handle, u32, *mut c_void, u32, *mut u32) -> i32;

    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
        fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
        fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
        fn K32GetProcessMemoryInfo(
            process: Handle,
            counters: *mut MemoryCounters,
            size: u32,
        ) -> i32;
        fn GetModuleHandleW(name: *const u16) -> Handle;
        fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
    }

    /// Closes the wrapped handle on drop.
    struct Owned(Handle);

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: the handle is valid and owned by this guard.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn open(access: u32, pid: u32) -> Result<Owned, i32> {
        // SAFETY: plain call; a null result is handled below.
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
        } else {
            Ok(Owned(handle))
        }
    }

    pub fn pid_alive(pid: u32) -> bool {
        match open(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, pid) {
            Ok(handle) => {
                // SAFETY: the guard holds a valid handle with SYNCHRONIZE access.
                let waited = unsafe { WaitForSingleObject(handle.0, 0) };
                waited == WAIT_TIMEOUT
            }
            Err(code) => code == ERROR_ACCESS_DENIED,
        }
    }

    fn snapshot() -> Vec<ProcessEntry> {
        // SAFETY: plain call; the result is checked.
        let handle = unsafe { CreateToolhelp32Snapshot(0x2, 0) };
        if handle.is_null() || handle as isize == -1 {
            return Vec::new();
        }
        let guard = Owned(handle);
        let mut entries = Vec::new();
        // SAFETY: ProcessEntry is plain data; zero is a valid bit pattern.
        let mut entry: ProcessEntry = unsafe { std::mem::zeroed() };
        entry.size = std::mem::size_of::<ProcessEntry>() as u32;
        // SAFETY: the snapshot is valid and entry.size is set.
        let mut more = unsafe { Process32FirstW(guard.0, &mut entry) } != 0;
        while more {
            entries.push(entry);
            // SAFETY: as above.
            more = unsafe { Process32NextW(guard.0, &mut entry) } != 0;
        }
        entries
    }

    pub fn process_table() -> Vec<(u32, u32, String)> {
        snapshot()
            .iter()
            .map(|entry| {
                let end = entry
                    .exe
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(entry.exe.len());
                (
                    entry.pid,
                    entry.parent,
                    String::from_utf16_lossy(&entry.exe[..end]),
                )
            })
            .collect()
    }

    fn nt_query() -> Option<NtQuery> {
        static QUERY: OnceLock<Option<usize>> = OnceLock::new();
        let address = (*QUERY.get_or_init(|| {
            let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
            // SAFETY: name is NUL-terminated; ntdll is always loaded.
            let module = unsafe { GetModuleHandleW(name.as_ptr()) };
            if module.is_null() {
                return None;
            }
            // SAFETY: the name is NUL-terminated.
            let found =
                unsafe { GetProcAddress(module, c"NtQueryInformationProcess".as_ptr().cast()) };
            (!found.is_null()).then_some(found as usize)
        }))?;
        // SAFETY: the address came from GetProcAddress for this exact signature.
        Some(unsafe { std::mem::transmute::<usize, NtQuery>(address) })
    }

    fn command_line(query: NtQuery, process: &Owned) -> Option<String> {
        let mut size: u32 = 512;
        for _ in 0..4 {
            // u64 elements keep the buffer aligned for UNICODE_STRING.
            let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
            let mut needed = 0u32;
            // SAFETY: buffer is at least `size` bytes and needed is writable.
            let status = unsafe {
                query(
                    process.0,
                    PROCESS_COMMAND_LINE_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    size,
                    &mut needed,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH
                || status == STATUS_BUFFER_OVERFLOW
                || status == STATUS_BUFFER_TOO_SMALL
            {
                size = needed.max(size.saturating_mul(2));
                continue;
            }
            if status < 0 {
                return None;
            }
            // UNICODE_STRING: u16 length (bytes), u16 max, padding, pointer.
            let base = buffer.as_ptr().cast::<u8>();
            // SAFETY: the kernel filled a UNICODE_STRING at the buffer start.
            let (length, pointer) = unsafe {
                (
                    usize::from(base.cast::<u16>().read()),
                    base.add(std::mem::size_of::<usize>())
                        .cast::<*const u16>()
                        .read(),
                )
            };
            if pointer.is_null() || length == 0 {
                return Some(String::new());
            }
            // SAFETY: the string points into our buffer and holds `length` bytes.
            let units = unsafe { std::slice::from_raw_parts(pointer, length / 2) };
            return Some(String::from_utf16_lossy(units));
        }
        None
    }

    fn rss_bytes(process: &Owned) -> u64 {
        // SAFETY: zero is valid for plain counters; cb is set before the call.
        let mut counters: MemoryCounters = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<MemoryCounters>() as u32;
        // SAFETY: the handle has query access and counters is writable.
        let ok = unsafe { K32GetProcessMemoryInfo(process.0, &mut counters, counters.cb) };
        if ok == 0 {
            0
        } else {
            counters.working_set_size as u64
        }
    }

    pub fn command_lines() -> Vec<ProcessInfo> {
        let Some(query) = nt_query() else {
            return Vec::new();
        };
        let mut infos = Vec::new();
        for entry in snapshot() {
            let Ok(process) = open(PROCESS_QUERY_LIMITED_INFORMATION, entry.pid) else {
                continue;
            };
            let Some(command_line) = command_line(query, &process) else {
                continue;
            };
            infos.push(ProcessInfo {
                pid: entry.pid,
                rss_bytes: rss_bytes(&process),
                command_line,
            });
        }
        infos
    }
}

#[cfg(windows)]
pub use win::{command_lines, pid_alive, process_table};

#[cfg(test)]
#[path = "procs_tests.rs"]
mod procs_tests;
