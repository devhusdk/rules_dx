//! Synchronous child lifecycle shared below domain adapters.
//!
//! Quality tool execution and streamed CLI/Bazel passthrough both spawn,
//! bound, and reap children. This module owns that mechanism once: the
//! caller describes the spawn, the environment shape, the capture bound,
//! and the deadline, and gets back a typed outcome. Parsing, policy, and
//! report rendering stay in the calling domain.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How the child sees the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvPolicy {
    /// Keep the parent environment and add these entries on top.
    /// Bazel passthrough uses this so terminal, signal, and auth state flow through.
    Inherited { extra: Vec<(OsString, OsString)> },
    /// Start from an empty environment with exactly these entries.
    /// Controlled tool execution uses this so only pinned values reach the tool.
    Controlled { vars: Vec<(OsString, OsString)> },
}

/// How much tool output the caller is willing to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePolicy {
    /// Per-stream byte bound supplied by the caller. The domain parser
    /// limit is passed in here, never imported by this module.
    pub max_bytes: usize,
}

/// Everything one spawn needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    /// The binary followed by its arguments. Empty is rejected before spawning.
    pub argv: Vec<OsString>,
    /// The directory the child starts in.
    pub cwd: PathBuf,
    /// Inherited or controlled environment.
    pub env: EnvPolicy,
    /// Per-stream capture bound.
    pub capture: CapturePolicy,
    /// Monotonic bound for the child to exit on its own. Cleanup and
    /// draining after the deadline are bounded separately below.
    pub timeout: Duration,
}

/// How a reaped child went out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

impl Exit {
    pub fn code(&self) -> Option<i32> {
        match self {
            Exit::Code(code) => Some(*code),
            Exit::Signal(_) => None,
        }
    }
}

/// What one bounded spawn produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildOutcome {
    Finished {
        exit: Exit,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut,
    OutputTooLarge {
        limit: usize,
    },
}

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const GRACEFUL_WAIT: Duration = Duration::from_secs(2);
const FORCED_WAIT: Duration = Duration::from_secs(2);
const DRAIN_WAIT: Duration = Duration::from_secs(5);

/// The process group of the operation this process currently owns, zero when idle.
static OWNED_GROUP: AtomicU32 = AtomicU32::new(0);
/// The last cancellation signal forwarded to an owned operation, zero when none.
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

/// Forwards one cancellation signal to the operation this process owns.
///
/// Only an atomic load and a process-group signal run here, so a signal
/// handler may call it. Returns false when no operation is owned or the
/// platform has no forwarding backend yet.
pub fn interrupt(signo: i32) -> bool {
    INTERRUPTED.store(signo, Ordering::SeqCst);
    let group = OWNED_GROUP.load(Ordering::SeqCst);
    group != 0 && backend::signal_group(group, signo)
}

/// The cancellation signal forwarded through [`interrupt`], if any.
pub fn interrupted() -> Option<i32> {
    match INTERRUPTED.load(Ordering::SeqCst) {
        0 => None,
        signo => Some(signo),
    }
}

/// Registration token for the owned process group; clears it on drop.
struct Ownership {
    group: u32,
}

impl Ownership {
    fn take(group: u32) -> Ownership {
        OWNED_GROUP.store(group, Ordering::SeqCst);
        Ownership { group }
    }
}

impl Drop for Ownership {
    fn drop(&mut self) {
        let _ = OWNED_GROUP.compare_exchange(self.group, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

/// Run one child to a bounded outcome.
///
/// The child waits at most `timeout`, then the owned tree is asked to stop
/// gracefully, then forced, then the pipes are drained. The call returns no
/// later than the timeout plus the graceful, forced, and drain bounds above.
/// Spawn and wait failures are I/O errors. A tree that escapes its owner is
/// reported as a timeout, never as success.
pub fn run(spec: &SpawnSpec) -> io::Result<ChildOutcome> {
    let (program, args) = spec
        .argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match &spec.env {
        EnvPolicy::Inherited { extra } => {
            command.envs(extra.iter().map(|(key, value)| (key, value)));
        }
        EnvPolicy::Controlled { vars } => {
            command.env_clear();
            command.envs(vars.iter().map(|(key, value)| (key, value)));
        }
    }
    let mut owned = backend::spawn_owned(&mut command)?;
    let _ownership = Ownership::take(backend::owned_group(&owned));
    let limit = spec.capture.max_bytes;
    let stdout_rx = drain_pipe(owned.take_stdout(), limit);
    let stderr_rx = drain_pipe(owned.take_stderr(), limit);
    let start = Instant::now();
    let soft = start + spec.timeout;
    let hard = soft + GRACEFUL_WAIT + FORCED_WAIT + DRAIN_WAIT;
    let mut status = None;
    let mut wait_error = None;
    while Instant::now() < soft {
        match owned.try_wait() {
            Ok(Some(exit)) => {
                status = Some(exit);
                break;
            }
            Ok(None) => {
                let remaining = soft.saturating_duration_since(Instant::now());
                std::thread::sleep(POLL_INTERVAL.min(remaining));
            }
            Err(err) => {
                wait_error = Some(err);
                break;
            }
        }
    }
    if let Some(err) = wait_error {
        backend::terminate_forced(&mut owned);
        let _ = poll_exit(&mut owned, hard);
        return Err(err);
    }
    match status {
        Some(exit) => {
            let stdout = collect(&stdout_rx, hard);
            let stderr = collect(&stderr_rx, hard);
            match (stdout, stderr) {
                (Drain::Ready(stdout), Drain::Ready(stderr)) => {
                    if stdout.len() > limit || stderr.len() > limit {
                        backend::terminate_forced(&mut owned);
                        Ok(ChildOutcome::OutputTooLarge { limit })
                    } else {
                        Ok(ChildOutcome::Finished {
                            exit: exit_of(&exit),
                            stdout,
                            stderr,
                        })
                    }
                }
                _ => {
                    backend::terminate_forced(&mut owned);
                    Ok(ChildOutcome::TimedOut)
                }
            }
        }
        None => {
            backend::terminate_graceful(&mut owned);
            let exit = poll_exit(&mut owned, hard.min(Instant::now() + GRACEFUL_WAIT));
            if exit.is_none() {
                backend::terminate_forced(&mut owned);
                let _ = poll_exit(&mut owned, hard.min(Instant::now() + FORCED_WAIT));
            }
            let _ = collect(&stdout_rx, hard);
            let _ = collect(&stderr_rx, hard);
            Ok(ChildOutcome::TimedOut)
        }
    }
}

/// How a streamed child attaches its standard streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    /// stdout and stderr inherit the caller's handles.
    Inherited,
    /// stderr inherits; stdout merges into the caller's stderr.
    StdoutToStderr,
}

/// Everything one streamed spawn needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSpec {
    /// The binary followed by its arguments. Empty is rejected before spawning.
    pub argv: Vec<OsString>,
    /// The directory the child starts in.
    pub cwd: PathBuf,
    /// Inherited or controlled environment.
    pub env: EnvPolicy,
    /// Where the child's standard output goes.
    pub streams: StreamMode,
    /// Bound for draining the streams once the child exits.
    pub drain: Duration,
}

/// Default drain bound for public streamed execution.
pub const STREAM_DRAIN_WAIT: Duration = DRAIN_WAIT;

/// What one streamed spawn produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamOutcome {
    /// The child ended and its streams drained inside the drain bound.
    Finished { exit: Exit },
    /// A forwarded cancellation stopped the owned tree before a clean end.
    Cancelled { exit: Option<Exit> },
    /// A descendant held a stream open past the drain bound.
    Undrained { exit: Exit },
}

/// Run one streamed child without a build-duration deadline.
///
/// The child keeps the caller's stdin and stderr, and stdout per
/// `spec.streams`. No fixed duration bound applies while it runs. After it
/// exits the streams drain at most `spec.drain`; a descendant that still
/// holds them makes the run stop the owned tree and report
/// [`StreamOutcome::Undrained`]. A signal forwarded through [`interrupt`]
/// stops the owned tree with bounded cleanup and reports
/// [`StreamOutcome::Cancelled`], never success.
pub fn run_streamed(spec: &StreamSpec) -> io::Result<StreamOutcome> {
    let (program, args) = spec
        .argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit());
    match spec.streams {
        StreamMode::Inherited => {
            command.stdout(Stdio::inherit());
        }
        StreamMode::StdoutToStderr => {
            command.stdout(Stdio::piped());
        }
    }
    match &spec.env {
        EnvPolicy::Inherited { extra } => {
            command.envs(extra.iter().map(|(key, value)| (key, value)));
        }
        EnvPolicy::Controlled { vars } => {
            command.env_clear();
            command.envs(vars.iter().map(|(key, value)| (key, value)));
        }
    }
    let mut owned = backend::spawn_owned(&mut command)?;
    let _ownership = Ownership::take(backend::owned_group(&owned));
    let (pump, drained_rx) = match spec.streams {
        StreamMode::Inherited => (None, None),
        StreamMode::StdoutToStderr => {
            let stdout = owned.take_stdout();
            let (tx, rx) = mpsc::channel();
            let pump = std::thread::spawn(move || {
                if let Some(mut stdout) = stdout {
                    let mut stderr = io::stderr();
                    let _ = io::copy(&mut stdout, &mut stderr);
                }
                let _ = tx.send(());
            });
            (Some(pump), Some(rx))
        }
    };
    let mut status = None;
    let mut wait_error = None;
    loop {
        match owned.try_wait() {
            Ok(Some(exit)) => {
                status = Some(exit);
                break;
            }
            Ok(None) => {
                if interrupted().is_some() {
                    backend::terminate_graceful(&mut owned);
                    status = poll_exit(&mut owned, Instant::now() + GRACEFUL_WAIT);
                    if status.is_none() {
                        backend::terminate_forced(&mut owned);
                        status = poll_exit(&mut owned, Instant::now() + FORCED_WAIT);
                    }
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => {
                wait_error = Some(err);
                break;
            }
        }
    }
    if let Some(err) = wait_error {
        backend::terminate_forced(&mut owned);
        let _ = poll_exit(&mut owned, Instant::now() + FORCED_WAIT);
        let _ = drain_within(&drained_rx, FORCED_WAIT);
        return Err(err);
    }
    let cancelled = interrupted().is_some();
    let mut drained = drain_within(&drained_rx, spec.drain);
    if !drained {
        backend::terminate_graceful(&mut owned);
        drained = drain_within(&drained_rx, GRACEFUL_WAIT);
        if !drained {
            backend::terminate_forced(&mut owned);
            drained = drain_within(&drained_rx, FORCED_WAIT);
        }
    }
    if let Some(pump) = pump {
        if drained {
            let _ = pump.join();
        }
    }
    let exit = status.map(|status| exit_of(&status));
    if cancelled {
        return Ok(StreamOutcome::Cancelled { exit });
    }
    match exit {
        Some(exit) if drained => Ok(StreamOutcome::Finished { exit }),
        Some(exit) => Ok(StreamOutcome::Undrained { exit }),
        None => Err(io::Error::other("streamed child could not be reaped")),
    }
}

fn drain_within(rx: &Option<mpsc::Receiver<()>>, bound: Duration) -> bool {
    match rx {
        None => true,
        Some(rx) => matches!(rx.recv_timeout(bound), Ok(())),
    }
}

fn exit_of(status: &ExitStatus) -> Exit {
    if let Some(code) = status.code() {
        Exit::Code(code)
    } else {
        Exit::Signal(backend::termination_signal(status).unwrap_or(0))
    }
}

fn poll_exit(owned: &mut backend::OwnedChild, until: Instant) -> Option<ExitStatus> {
    loop {
        match owned.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                let now = Instant::now();
                if now >= until {
                    return None;
                }
                std::thread::sleep(POLL_INTERVAL.min(until - now));
            }
            Err(_) => return None,
        }
    }
}

enum Drain {
    Ready(Vec<u8>),
    Late,
    Panicked,
}

fn collect(rx: &mpsc::Receiver<Vec<u8>>, until: Instant) -> Drain {
    let remaining = until.saturating_duration_since(Instant::now());
    match rx.recv_timeout(remaining) {
        Ok(buf) => Drain::Ready(buf),
        Err(mpsc::RecvTimeoutError::Timeout) => Drain::Late,
        Err(mpsc::RecvTimeoutError::Disconnected) => Drain::Panicked,
    }
}

fn drain_pipe<R: io::Read + Send + 'static>(
    pipe: Option<R>,
    limit: usize,
) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    match pipe {
        None => {
            let _ = tx.send(Vec::new());
        }
        Some(mut pipe) => {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 16 * 1024];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            let room = limit.saturating_add(1).saturating_sub(buf.len());
                            buf.extend_from_slice(&chunk[..n.min(room)]);
                        }
                        Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                let _ = tx.send(buf);
            });
        }
    }
    rx
}

mod backend {
    #[cfg(unix)]
    mod unix {
        use super::super::{Command, ExitStatus};
        use std::io;
        use std::process::{ChildStderr, ChildStdout};

        const SIGTERM: i32 = 15;
        const SIGKILL: i32 = 9;

        unsafe extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }

        pub struct OwnedChild {
            child: std::process::Child,
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

        pub fn spawn_owned(command: &mut Command) -> io::Result<OwnedChild> {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            Ok(OwnedChild {
                child: command.spawn()?,
            })
        }

        pub fn owned_group(owned: &OwnedChild) -> u32 {
            owned.child.id()
        }

        pub fn signal_group(group: u32, signo: i32) -> bool {
            let pgid = -(group as i32);
            unsafe { kill(pgid, signo) == 0 }
        }

        pub fn terminate_graceful(owned: &mut OwnedChild) {
            let _ = signal_group(owned.child.id(), SIGTERM);
        }

        pub fn terminate_forced(owned: &mut OwnedChild) {
            let _ = signal_group(owned.child.id(), SIGKILL);
        }

        pub fn termination_signal(status: &ExitStatus) -> Option<i32> {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        }
    }

    #[cfg(windows)]
    mod windows {
        use super::super::{Command, ExitStatus};
        use std::ffi::c_void;
        use std::io;
        use std::os::windows::io::AsRawHandle;
        use std::process::{ChildStderr, ChildStdout};

        type Handle = *mut c_void;

        const KILL_ON_JOB_CLOSE: u32 = 0x2000;
        const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;

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
            fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
            fn SetInformationJobObject(
                handle: Handle,
                class: i32,
                info: *const c_void,
                len: u32,
            ) -> i32;
            fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
            fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
            fn CloseHandle(handle: Handle) -> i32;
        }

        struct Job {
            handle: Handle,
        }

        impl Job {
            fn create() -> io::Result<Job> {
                let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
                if handle.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let mut info: ExtendedLimit = unsafe { std::mem::zeroed() };
                info.basic.limit_flags = KILL_ON_JOB_CLOSE;
                let applied = unsafe {
                    SetInformationJobObject(
                        handle,
                        JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                        std::ptr::addr_of!(info) as *const c_void,
                        std::mem::size_of::<ExtendedLimit>() as u32,
                    )
                };
                if applied == 0 {
                    let err = io::Error::last_os_error();
                    unsafe {
                        CloseHandle(handle);
                    }
                    return Err(err);
                }
                Ok(Job { handle })
            }

            fn assign(&self, child: &std::process::Child) -> io::Result<()> {
                let process = child.as_raw_handle() as Handle;
                let assigned = unsafe { AssignProcessToJobObject(self.handle, process) };
                if assigned == 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            }

            fn terminate(&self) {
                unsafe {
                    TerminateJobObject(self.handle, 1);
                }
            }
        }

        impl Drop for Job {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.handle);
                }
            }
        }

        pub struct OwnedChild {
            child: std::process::Child,
            _job: Job,
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

        pub fn spawn_owned(command: &mut Command) -> io::Result<OwnedChild> {
            let job = Job::create()?;
            match command.spawn() {
                Ok(mut child) => {
                    if let Err(err) = job.assign(&child) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(err);
                    }
                    Ok(OwnedChild { child, _job: job })
                }
                Err(err) => Err(err),
            }
        }

        pub fn owned_group(owned: &OwnedChild) -> u32 {
            owned.child.id()
        }

        /// Native console control for an owned job arrives with its own evidence.
        pub fn signal_group(_group: u32, _signo: i32) -> bool {
            false
        }

        pub fn terminate_graceful(_owned: &mut OwnedChild) {}

        pub fn terminate_forced(owned: &mut OwnedChild) {
            owned._job.terminate();
        }

        pub fn termination_signal(_status: &ExitStatus) -> Option<i32> {
            None
        }
    }

    #[cfg(unix)]
    pub use unix::{
        owned_group, signal_group, spawn_owned, terminate_forced, terminate_graceful,
        termination_signal, OwnedChild,
    };
    #[cfg(windows)]
    pub use windows::{
        owned_group, signal_group, spawn_owned, terminate_forced, terminate_graceful,
        termination_signal, OwnedChild,
    };

    #[cfg(not(any(unix, windows)))]
    mod portable {
        use super::super::{Command, ExitStatus};
        use std::io;
        use std::process::{ChildStderr, ChildStdout};

        pub struct OwnedChild {
            child: std::process::Child,
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

        pub fn spawn_owned(command: &mut Command) -> io::Result<OwnedChild> {
            Ok(OwnedChild {
                child: command.spawn()?,
            })
        }

        pub fn owned_group(owned: &OwnedChild) -> u32 {
            owned.child.id()
        }

        pub fn signal_group(_group: u32, _signo: i32) -> bool {
            false
        }

        pub fn terminate_graceful(_owned: &mut OwnedChild) {}

        pub fn terminate_forced(owned: &mut OwnedChild) {
            let _ = owned.child.kill();
        }

        pub fn termination_signal(_status: &ExitStatus) -> Option<i32> {
            None
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub use portable::{
        owned_group, signal_group, spawn_owned, terminate_forced, terminate_graceful,
        termination_signal, OwnedChild,
    };
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
