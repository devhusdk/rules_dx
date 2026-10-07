//! Typed child lifecycle primitives shared by dx process launchers.

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const TERM_GRACE: Duration = Duration::from_millis(500);

/// Whether the child inherits stdin or reads end-of-file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinPolicy {
    Inherit,
    Closed,
}

/// Whether the run owns the lifetime of the child's descendants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreePolicy {
    Inherited,
    Owned,
}

/// The environment a child receives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvPolicy {
    /// The parent environment plus the extra pairs.
    Inherited { extra: Vec<(OsString, OsString)> },
    /// Only the listed pairs; nothing is inherited.
    Controlled { vars: Vec<(OsString, OsString)> },
}

/// Caller-provided output byte bound, applied to both streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturePolicy {
    pub max_output_bytes: usize,
}

/// The fixed inputs of one child run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub stdin: StdinPolicy,
    pub tree: TreePolicy,
    pub timeout: Option<Duration>,
}

/// How a child exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Code(i32),
    Signaled { signal: i32 },
}

/// Where a failed run broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    Spawn,
    Io,
}

/// The result of one child run.
#[derive(Debug)]
pub enum ChildOutcome {
    Completed {
        exit: ExitKind,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut,
    Failed {
        stage: FailureStage,
        error: io::Error,
    },
}

/// Runs one child to completion, timeout, or failure.
pub fn run(spec: &SpawnSpec, env: &EnvPolicy, capture: &CapturePolicy) -> ChildOutcome {
    let Some((binary, args)) = spec.argv.split_first() else {
        return ChildOutcome::Failed {
            stage: FailureStage::Spawn,
            error: io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"),
        };
    };
    let mut command = Command::new(binary);
    command.args(args).current_dir(&spec.cwd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    match spec.stdin {
        StdinPolicy::Inherit => {
            command.stdin(Stdio::inherit());
        }
        StdinPolicy::Closed => {
            command.stdin(Stdio::piped());
        }
    }
    match env {
        EnvPolicy::Inherited { extra } => {
            command.envs(extra.iter().map(|(key, value)| (key, value)));
        }
        EnvPolicy::Controlled { vars } => {
            command.env_clear();
            command.envs(vars.iter().map(|(key, value)| (key, value)));
        }
    }
    let spawned = match spec.tree {
        TreePolicy::Inherited => command.spawn().map(|child| (child, Tree::Inherited)),
        TreePolicy::Owned => spawn_owned(&mut command),
    };
    let (mut child, tree) = match spawned {
        Ok(pair) => pair,
        Err(error) => {
            return ChildOutcome::Failed {
                stage: FailureStage::Spawn,
                error,
            }
        }
    };
    if matches!(spec.stdin, StdinPolicy::Closed) {
        drop(child.stdin.take());
    }
    let mut stdout_reader = child
        .stdout
        .take()
        .map(|pipe| drain_pipe(pipe, capture.max_output_bytes));
    let mut stderr_reader = child
        .stderr
        .take()
        .map(|pipe| drain_pipe(pipe, capture.max_output_bytes));
    let deadline = spec.timeout.map(|timeout| Instant::now() + timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    stop_child(&mut child, &tree, true);
                    let _ = join_drain(stdout_reader.take());
                    let _ = join_drain(stderr_reader.take());
                    return ChildOutcome::TimedOut;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                stop_child(&mut child, &tree, false);
                let _ = join_drain(stdout_reader.take());
                let _ = join_drain(stderr_reader.take());
                return ChildOutcome::Failed {
                    stage: FailureStage::Io,
                    error,
                };
            }
        }
    };
    if deadline.is_some() {
        while !drains_finished(&stdout_reader, &stderr_reader) {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                tree.terminate_forced();
                let _ = join_drain(stdout_reader.take());
                let _ = join_drain(stderr_reader.take());
                return ChildOutcome::TimedOut;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
    let stdout = match join_drain(stdout_reader.take()) {
        Ok(bytes) => bytes,
        Err(error) => {
            tree.terminate_forced();
            let _ = join_drain(stderr_reader.take());
            return ChildOutcome::Failed {
                stage: FailureStage::Io,
                error,
            };
        }
    };
    let stderr = match join_drain(stderr_reader.take()) {
        Ok(bytes) => bytes,
        Err(error) => {
            tree.terminate_forced();
            return ChildOutcome::Failed {
                stage: FailureStage::Io,
                error,
            };
        }
    };
    if stdout.len() > capture.max_output_bytes || stderr.len() > capture.max_output_bytes {
        tree.terminate_forced();
        let limit = capture.max_output_bytes;
        return ChildOutcome::Failed {
            stage: FailureStage::Io,
            error: io::Error::new(
                io::ErrorKind::InvalidData,
                format!("tool output exceeds max size {limit} bytes"),
            ),
        };
    }
    ChildOutcome::Completed {
        exit: classify(&status),
        stdout,
        stderr,
    }
}

/// Maps a wait status onto the typed exit kind.
pub(crate) fn classify(status: &ExitStatus) -> ExitKind {
    match status.code() {
        Some(code) => ExitKind::Code(code),
        None => match exit_signal(status) {
            Some(signal) => ExitKind::Signaled { signal },
            None => ExitKind::Code(0),
        },
    }
}

/// Rebuilds a wait status from the typed exit kind.
pub(crate) fn status_from(exit: &ExitKind) -> ExitStatus {
    match exit {
        ExitKind::Code(code) => ExitStatus::from_raw(raw_code(*code)),
        ExitKind::Signaled { signal } => ExitStatus::from_raw(raw_signal(*signal)),
    }
}

#[cfg(unix)]
fn raw_code(code: i32) -> i32 {
    (code & 0xff) << 8
}

#[cfg(not(unix))]
fn raw_code(code: i32) -> i32 {
    code
}

#[cfg(unix)]
fn raw_signal(signal: i32) -> i32 {
    signal & 0x7f
}

#[cfg(not(unix))]
fn raw_signal(signal: i32) -> i32 {
    signal
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    let _ = status;
    None
}

fn stop_child(child: &mut Child, tree: &Tree, graceful: bool) {
    if graceful && tree.terminate_graceful() {
        let grace_deadline = Instant::now() + TERM_GRACE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < grace_deadline => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Ok(None) | Err(_) => break,
            }
        }
    }
    tree.terminate_forced();
    let _ = child.kill();
    let _ = child.wait();
}

fn drains_finished(
    stdout: &Option<std::thread::JoinHandle<Vec<u8>>>,
    stderr: &Option<std::thread::JoinHandle<Vec<u8>>>,
) -> bool {
    stdout.as_ref().is_none_or(|handle| handle.is_finished())
        && stderr.as_ref().is_none_or(|handle| handle.is_finished())
}

fn drain_pipe<R: Read + Send + 'static>(mut pipe: R, limit: usize) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let room = limit.saturating_add(1).saturating_sub(buf.len());
                    let take = n.min(room);
                    buf.extend_from_slice(&chunk[..take]);
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        buf
    })
}

fn join_drain(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> io::Result<Vec<u8>> {
    match handle {
        None => Ok(Vec::new()),
        Some(handle) => handle
            .join()
            .map_err(|_| io::Error::other("tool output reader thread panicked")),
    }
}

/// The process tree handle a run keeps alive.
#[derive(Debug)]
enum Tree {
    Inherited,
    #[cfg(unix)]
    OwnedGroup { pgid: i32 },
    #[cfg(windows)]
    OwnedJob { job: JobHandle },
}

impl Tree {
    fn terminate_graceful(&self) -> bool {
        match self {
            Tree::Inherited => false,
            #[cfg(unix)]
            Tree::OwnedGroup { pgid } => unsafe { libc::killpg(*pgid, libc::SIGTERM) } == 0,
            #[cfg(windows)]
            Tree::OwnedJob { .. } => false,
        }
    }

    fn terminate_forced(&self) {
        match self {
            Tree::Inherited => {}
            #[cfg(unix)]
            Tree::OwnedGroup { pgid } => {
                let _ = unsafe { libc::killpg(*pgid, libc::SIGKILL) };
            }
            #[cfg(windows)]
            Tree::OwnedJob { job } => {
                let _ = unsafe {
                    windows_sys::Win32::System::JobObjects::TerminateJobObject(job.handle, 1)
                };
            }
        }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        self.terminate_forced();
    }
}

#[cfg(unix)]
fn spawn_owned(command: &mut Command) -> io::Result<(Child, Tree)> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    let child = command.spawn()?;
    let pgid = child.id() as i32;
    Ok((child, Tree::OwnedGroup { pgid }))
}

#[cfg(windows)]
fn spawn_owned(command: &mut Command) -> io::Result<(Child, Tree)> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;
    let job = JobHandle::create()?;
    command.creation_flags(CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    if let Err(error) = assign_and_resume(&child, &job) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok((child, Tree::OwnedJob { job }))
}

#[cfg(windows)]
fn assign_and_resume(child: &Child, job: &JobHandle) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    if unsafe { AssignProcessToJobObject(job.handle, child.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    resume_threads(child.id())
}

#[cfg(windows)]
fn resume_threads(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut resumed = 0u32;
    if unsafe { Thread32First(snapshot, &mut entry) } != 0 {
        loop {
            if entry.th32OwnerProcessID == pid {
                let thread =
                    unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    unsafe { CloseHandle(snapshot) };
                    return Err(io::Error::last_os_error());
                }
                let previous = unsafe { ResumeThread(thread) };
                unsafe { CloseHandle(thread) };
                if previous == u32::MAX {
                    unsafe { CloseHandle(snapshot) };
                    return Err(io::Error::last_os_error());
                }
                resumed += 1;
            }
            if unsafe { Thread32Next(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snapshot) };
    if resumed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "child thread to resume was not found",
        ));
    }
    Ok(())
}

#[cfg(windows)]
struct JobHandle {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl JobHandle {
    fn create() -> io::Result<JobHandle> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let applied = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if applied == 0 {
            let error = io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(error);
        }
        Ok(JobHandle { handle })
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
