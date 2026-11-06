use super::{Command, Descendants, ExitStatus};
use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, ChildStderr, ChildStdout};

type Handle = *mut c_void;

pub(crate) unsafe extern "system" fn console_stop(ctrl_type: u32) -> i32 {
    match super::console_stop_signal(ctrl_type) {
        Some(signo) => {
            crate::record_interrupt(signo);
            1
        }
        None => 0,
    }
}

/// Makes Ctrl-C and Ctrl-Break record a stop like SIGINT. Every process on the console gets the event itself, so nothing is forwarded.
pub fn install_console_stop() {
    // SAFETY: registers a static function that only records the stop.
    unsafe {
        // A parent that started dx with Ctrl-C disabled passes that on; dx wants the event.
        SetConsoleCtrlHandler(None, 0);
        SetConsoleCtrlHandler(Some(console_stop), 1);
    }
}

const KILL_ON_JOB_CLOSE: u32 = 0x2000;
const JOB_OBJECT_LIMIT_BREAKAWAY_OK: u32 = 0x800;
const CREATE_SUSPENDED: u32 = 0x4;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
const ERROR_ACCESS_DENIED: i32 = 5;
const TH32CS_SNAPTHREAD: u32 = 0x4;
const THREAD_SUSPEND_RESUME: u32 = 0x2;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
const JOB_OBJECT_BASIC_PROCESS_ID_LIST: i32 = 3;
const PROCESS_TERMINATE: u32 = 0x1;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const ERROR_MORE_DATA: i32 = 234;
const LIST_ATTEMPTS: usize = 8;

#[repr(C)]
struct BasicLimit {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct ExtendedLimit {
    basic: BasicLimit,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

unsafe extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(handle: Handle, class: i32, info: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
    fn QueryInformationJobObject(
        job: Handle,
        class: i32,
        info: *mut c_void,
        len: u32,
        returned: *mut u32,
    ) -> i32;
    fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> Handle;
    fn IsProcessInJob(process: Handle, job: Handle, result: *mut i32) -> i32;
    fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> Handle;
    fn ResumeThread(thread: Handle) -> u32;
}

#[repr(C)]
struct ThreadEntry {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

fn resume_threads(pid: u32) -> io::Result<()> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot.is_null() || snapshot as isize == -1 {
        return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    let mut entry: ThreadEntry = unsafe { std::mem::zeroed() };
    entry.size = std::mem::size_of::<ThreadEntry>() as u32;
    let mut resumed = 0usize;
    let mut failure = None;
    let mut more = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while more {
        if entry.owner_process_id == pid {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.thread_id) };
            if thread.is_null() {
                // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
                failure = Some(io::Error::last_os_error());
                break;
                // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            }
            let previous = unsafe { ResumeThread(thread) };
            let err = io::Error::last_os_error();
            unsafe {
                CloseHandle(thread);
            }
            if previous == u32::MAX {
                // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
                failure = Some(err);
                break;
                // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            }
            resumed += 1;
        }
        more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    unsafe {
        CloseHandle(snapshot);
    }
    match failure {
        // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        Some(err) => Err(err),
        None if resumed == 0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the suspended child has no thread",
        )),
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        None => Ok(()),
    }
}

fn console_members() -> Vec<u32> {
    let mut list = vec![0u32; 64];
    for _ in 0..LIST_ATTEMPTS {
        let count = unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) } as usize;
        if count <= list.len() {
            list.truncate(count);
            return list;
            // LCOV_EXCL_START - reason: the list outgrowing its buffer between two calls, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        list.resize(count + 16, 0);
    }
    Vec::new()
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
}

struct Job {
    handle: Handle,
    descendants: Descendants,
}

impl Job {
    fn create(descendants: Descendants) -> io::Result<Job> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        let mut info: ExtendedLimit = unsafe { std::mem::zeroed() };
        info.basic.limit_flags = match descendants {
            Descendants::Owned => KILL_ON_JOB_CLOSE,
            Descendants::Detachable => JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        };
        let applied = unsafe {
            SetInformationJobObject(
                handle,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                std::ptr::addr_of!(info) as *const c_void,
                std::mem::size_of::<ExtendedLimit>() as u32,
            )
        };
        if applied == 0 {
            // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            let err = io::Error::last_os_error();
            unsafe {
                CloseHandle(handle);
            }
            return Err(err);
            // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(Job {
            handle,
            descendants,
        })
    }

    fn assign(&self, child: &std::process::Child) -> io::Result<()> {
        let process = child.as_raw_handle() as Handle;
        let assigned = unsafe { AssignProcessToJobObject(self.handle, process) };
        if assigned == 0 {
            Err(io::Error::last_os_error()) // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        } else {
            Ok(())
        }
    }

    fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.handle, 1);
        }
    }

    fn members(&self) -> Option<Vec<u32>> {
        let header = 8 / std::mem::size_of::<usize>();
        let mut capacity = 64usize;
        for _ in 0..LIST_ATTEMPTS {
            let mut buffer = vec![0usize; header + capacity];
            let queried = unsafe {
                QueryInformationJobObject(
                    self.handle,
                    JOB_OBJECT_BASIC_PROCESS_ID_LIST,
                    buffer.as_mut_ptr() as *mut c_void,
                    (buffer.len() * std::mem::size_of::<usize>()) as u32,
                    std::ptr::null_mut(),
                )
            } != 0;
            let err = io::Error::last_os_error();
            let counts = buffer.as_ptr() as *const u32;
            let (assigned, listed) = unsafe { (*counts as usize, *counts.add(1) as usize) };
            if queried && assigned <= listed {
                let listed = listed.min(capacity);
                return Some(
                    buffer[header..header + listed]
                        .iter()
                        .map(|&pid| pid as u32)
                        .collect(),
                );
                // LCOV_EXCL_START - reason: the list outgrowing its buffer between two calls, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            }
            if !queried && err.raw_os_error() != Some(ERROR_MORE_DATA) {
                return None;
            }
            capacity = (assigned + 16).max(capacity * 2);
        }
        None
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }

    fn kill_member(&self, pid: u32) {
        let process = unsafe {
            OpenProcess(
                PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        if process.is_null() {
            return; // LCOV_EXCL_LINE - reason: the member exited before it was opened, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        let mut inside = 0;
        unsafe {
            if IsProcessInJob(process, self.handle, &mut inside) != 0 && inside != 0 {
                TerminateProcess(process, 1);
            }
            CloseHandle(process);
        }
    }

    fn stop_attached(&self) -> bool {
        let members = self.members();
        let console = console_members();
        let targets = super::attached_targets(std::process::id(), members.as_deref(), &console);
        for pid in targets.iter().flatten() {
            self.kill_member(*pid);
        }
        targets.is_some()
    }

    fn stop(&self) {
        // Without a console this ends the whole job, server included; see
        // docs/cli/commands/completion.md.
        if self.descendants == Descendants::Owned || !self.stop_attached() {
            self.terminate();
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.descendants == Descendants::Detachable {
            // No console: nothing is killed, so a normal exit leaves the server alone.
            self.stop_attached();
        }
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub struct OwnedChild {
    child: std::process::Child,
    job: Job,
}

impl OwnedChild {
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }
}

pub fn spawn_owned(command: &mut Command, descendants: Descendants) -> io::Result<OwnedChild> {
    use std::os::windows::process::CommandExt;
    let job = Job::create(descendants)?;
    let mut child = match descendants {
        Descendants::Owned => command.creation_flags(CREATE_SUSPENDED).spawn()?,
        Descendants::Detachable => match command
            .creation_flags(CREATE_SUSPENDED | CREATE_BREAKAWAY_FROM_JOB)
            .spawn()
        {
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
                command.creation_flags(CREATE_SUSPENDED).spawn()?
            }
            other => other?, // LCOV_EXCL_LINE - reason: bazel test runs inside a job that forbids breakaway, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        },
    };
    let started = job.assign(&child).and_then(|()| resume_threads(child.id()));
    if let Err(err) = started {
        // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        job.terminate();
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    Ok(OwnedChild { child, job })
}

/// Keeps a streamed child's kill-on-close job open while the run lasts.
pub struct StreamTree {
    _job: Option<Job>,
}

/// Spawns a streamed child; an owned or lock-holding child runs in a
/// kill-on-close job so its tree never outlives the run.
pub fn spawn_streamed(
    command: &mut Command,
    group: super::backend::Group,
    held: &[std::fs::File],
) -> io::Result<(std::process::Child, StreamTree)> {
    use std::os::windows::process::CommandExt;
    if held.is_empty() && !matches!(group, super::backend::Group::Owned) {
        return Ok((command.spawn()?, StreamTree { _job: None }));
    }
    let job = Job::create(Descendants::Owned)?;
    let mut child = command.creation_flags(CREATE_SUSPENDED).spawn()?;
    let started = job.assign(&child).and_then(|()| resume_threads(child.id()));
    if let Err(err) = started {
        // LCOV_EXCL_START - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        job.terminate();
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    Ok((child, StreamTree { _job: Some(job) }))
}

pub fn terminate_graceful(owned: &mut OwnedChild) {
    terminate_forced(owned);
}

pub fn terminate_forced(owned: &mut OwnedChild) {
    owned.job.stop();
    let _ = owned.child.kill();
}

pub fn termination_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

pub fn stream_target(_child: &Child, _group: super::backend::Group) -> i32 {
    0
}

pub fn send(_target: i32, _signo: i32) {}

pub fn send_kill(_target: i32) {}

pub fn force_stream(child: &mut Child, _target: i32) {
    let _ = child.kill();
}

pub fn tree_alive(_target: i32) -> bool {
    false
}

pub fn terminal_foreground() -> bool {
    true
}

pub fn allow_background_tty_writes() {}

/// No terminal job on Windows; streamed children share the console.
pub struct TerminalJob;

impl TerminalJob {
    pub fn start() -> Option<TerminalJob> {
        None
    }

    pub fn pgid(&self) -> i32 {
        0
    }

    pub fn take_reported(&self) -> Option<i32> {
        None
    }

    pub fn tend(&self, _child: i32) {}
}

pub fn end_owned_descendants(_owned: &mut OwnedChild) {}
