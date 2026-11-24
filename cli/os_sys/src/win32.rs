#![expect(unsafe_code, reason = "these Win32 process calls have no safe std API")]

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, HandleOrInvalid, HandleOrNull, OwnedHandle,
};
use std::path::PathBuf;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    FILETIME, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, NTSTATUS, SetHandleInformation,
    WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::SystemInformation::{
    GetSystemDirectoryW, GetSystemWindowsDirectoryW, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, INFINITE, OpenProcess, OpenThread, PROCESS_DUP_HANDLE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, ResumeThread, THREAD_SUSPEND_RESUME, TerminateProcess, WaitForSingleObject,
};

pub(crate) fn struct_size<T>() -> io::Result<u32> {
    u32::try_from(size_of::<T>()).map_err(io::Error::other)
}

#[link(name = "ntdll", kind = "raw-dylib")]
unsafe extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> NTSTATUS;
    fn NtResumeProcess(process: HANDLE) -> NTSTATUS;
}

pub use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_SUSPENDED};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Query,
    Wait,
    Terminate,
    SuspendResume,
    Duplicate,
}

impl Access {
    fn rights(self) -> u32 {
        match self {
            Access::Query => PROCESS_QUERY_LIMITED_INFORMATION,
            Access::Wait => PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            Access::Terminate => PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            Access::SuspendResume => PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SUSPEND_RESUME,
            Access::Duplicate => PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_DUP_HANDLE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent: u32,
    pub exe: String,
}

pub(crate) fn owned_or_last_error<H: TryInto<OwnedHandle>>(handle: H) -> io::Result<OwnedHandle> {
    match handle.try_into() {
        Ok(handle) => Ok(handle),
        Err(_) => Err(io::Error::last_os_error()),
    }
}

pub fn open(pid: u32, access: Access) -> io::Result<OwnedHandle> {
    // SAFETY: OpenProcess takes plain integers.
    let raw = unsafe { OpenProcess(access.rights(), 0, pid) };
    // SAFETY: OpenProcess returns a new handle owned by the caller, or null.
    let handle = unsafe { HandleOrNull::from_raw_handle(raw) };
    owned_or_last_error(handle)
}

pub fn is_running(process: BorrowedHandle<'_>) -> io::Result<bool> {
    wait(process, Some(Duration::ZERO)).map(|signaled| !signaled)
}

pub fn wait(object: BorrowedHandle<'_>, timeout: Option<Duration>) -> io::Result<bool> {
    let millis = match timeout {
        None => INFINITE,
        Some(timeout) => u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX)
            .min(INFINITE - 1),
    };
    // SAFETY: object is a live handle.
    let waited = unsafe { WaitForSingleObject(object.as_raw_handle(), millis) };
    match waited {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        WAIT_FAILED => Err(io::Error::last_os_error()),
        other => Err(io::Error::other(format!(
            "unexpected wait result {other:#x}"
        ))),
    }
}

pub fn terminate(pid: u32, exit_code: u32) -> io::Result<()> {
    let process = open(pid, Access::Terminate)?;
    // SAFETY: process is a live handle with PROCESS_TERMINATE access.
    let ok = unsafe { TerminateProcess(process.as_raw_handle(), exit_code) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn nt_status(what: &str, pid: u32, status: NTSTATUS) -> io::Result<()> {
    if status < 0 {
        return Err(io::Error::other(format!(
            "{what} of process {pid} failed with NTSTATUS {status:#x}"
        )));
    }
    Ok(())
}

pub fn suspend(pid: u32) -> io::Result<()> {
    let process = open(pid, Access::SuspendResume)?;
    // SAFETY: process is a live handle with PROCESS_SUSPEND_RESUME access.
    let status = unsafe { NtSuspendProcess(process.as_raw_handle()) };
    nt_status("NtSuspendProcess", pid, status)
}

pub fn resume(pid: u32) -> io::Result<()> {
    let process = open(pid, Access::SuspendResume)?;
    // SAFETY: process is a live handle with PROCESS_SUSPEND_RESUME access.
    let status = unsafe { NtResumeProcess(process.as_raw_handle()) };
    nt_status("NtResumeProcess", pid, status)
}

pub fn disinherit_std_handles() -> io::Result<()> {
    let which: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
    for std_handle in which {
        // SAFETY: GetStdHandle takes a plain constant.
        let handle = unsafe { GetStdHandle(std_handle) };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        if handle.is_null() {
            continue;
        }
        // SAFETY: handle is this process's standard handle, which stays open for the call.
        let ok = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn exit_code(process: BorrowedHandle<'_>) -> io::Result<u32> {
    let mut code = 0u32;
    // SAFETY: process is a live handle and code is writable.
    let ok = unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(code)
}

pub fn resume_thread(thread: BorrowedHandle<'_>) -> io::Result<()> {
    // SAFETY: thread is a live handle.
    let previous = unsafe { ResumeThread(thread.as_raw_handle()) };
    if previous == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn creation_time(process: BorrowedHandle<'_>) -> io::Result<u64> {
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut times = [zero; 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: process is a live handle and every pointer names a writable FILETIME.
    let ok = unsafe { GetProcessTimes(process.as_raw_handle(), creation, exit, kernel, user) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

pub fn working_set_bytes(process: BorrowedHandle<'_>) -> io::Result<u64> {
    let size = struct_size::<PROCESS_MEMORY_COUNTERS>()?;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        ..Default::default()
    };
    // SAFETY: process is a live handle and counters is writable for `size` bytes.
    let ok = unsafe { K32GetProcessMemoryInfo(process.as_raw_handle(), &mut counters, size) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    u64::try_from(counters.WorkingSetSize).map_err(io::Error::other)
}

fn snapshot(flags: u32) -> io::Result<OwnedHandle> {
    // SAFETY: CreateToolhelp32Snapshot takes plain integers.
    let raw = unsafe { CreateToolhelp32Snapshot(flags, 0) };
    // SAFETY: the snapshot is a new handle owned by the caller, or INVALID_HANDLE_VALUE.
    let handle = unsafe { HandleOrInvalid::from_raw_handle(raw) };
    owned_or_last_error(handle)
}

pub fn processes() -> io::Result<Vec<ProcessEntry>> {
    let snapshot = snapshot(TH32CS_SNAPPROCESS)?;
    let mut entry = PROCESSENTRY32W {
        dwSize: struct_size::<PROCESSENTRY32W>()?,
        ..Default::default()
    };
    let mut entries = Vec::new();
    // SAFETY: snapshot is a live process snapshot and entry has its size set.
    let mut more = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) } != 0;
    while more {
        let exe: Vec<u16> = entry
            .szExeFile
            .iter()
            .copied()
            .take_while(|unit| *unit != 0)
            .collect();
        entries.push(ProcessEntry {
            pid: entry.th32ProcessID,
            parent: entry.th32ParentProcessID,
            exe: String::from_utf16_lossy(&exe),
        });
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    Ok(entries)
}

fn open_thread(thread_id: u32) -> io::Result<OwnedHandle> {
    // SAFETY: OpenThread takes plain integers.
    let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    // SAFETY: OpenThread returns a new handle owned by the caller, or null.
    let handle = unsafe { HandleOrNull::from_raw_handle(raw) };
    owned_or_last_error(handle)
}

pub fn resume_threads(pid: u32) -> io::Result<()> {
    let snapshot = snapshot(TH32CS_SNAPTHREAD)?;
    let mut entry = THREADENTRY32 {
        dwSize: struct_size::<THREADENTRY32>()?,
        ..Default::default()
    };
    let mut resumed = 0usize;
    // SAFETY: snapshot is a live thread snapshot and entry has its size set.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            let thread = open_thread(entry.th32ThreadID)?;
            resume_thread(thread.as_handle())?;
            resumed += 1;
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("process {pid} has no thread"),
        ));
    }
    Ok(())
}

pub fn total_physical_memory() -> io::Result<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: struct_size::<MEMORYSTATUSEX>()?,
        ..Default::default()
    };
    // SAFETY: status is writable and its length field is set.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    Ok(status.ullTotalPhys)
}

pub fn system_dirs() -> io::Result<[PathBuf; 2]> {
    Ok([
        // SAFETY: wide_dir passes a writable buffer of the length it names.
        wide_dir(|buffer, len| unsafe { GetSystemDirectoryW(buffer, len) })?,
        // SAFETY: as above.
        wide_dir(|buffer, len| unsafe { GetSystemWindowsDirectoryW(buffer, len) })?,
    ])
}

fn wide_dir(get: impl Fn(*mut u16, u32) -> u32) -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 260];
    loop {
        let len = u32::try_from(buffer.len()).map_err(io::Error::other)?;
        let written = usize::try_from(get(buffer.as_mut_ptr(), len)).map_err(io::Error::other)?;
        if written == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        if written < buffer.len() {
            buffer.truncate(written);
            return Ok(PathBuf::from(OsString::from_wide(&buffer)));
        }
        buffer.resize(written, 0); // LCOV_EXCL_LINE - reason: the Windows directories fit in MAX_PATH, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
}
