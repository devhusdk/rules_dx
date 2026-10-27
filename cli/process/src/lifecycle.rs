//! Synchronous child lifecycle shared below domain adapters.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
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

/// Whether processes the child starts stay owned by the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descendants {
    /// Every descendant ends with the run.
    Owned,
    /// Descendants that leave the console or session, like a Bazel server, may outlive the run.
    /// On Windows, a forced stop without a console also ends them (see `attached_targets`).
    Detachable,
}

/// The job members a Detachable stop kills: those on dx's console, except dx itself.
/// `None` means the attached members cannot be told apart (no member list, or dx has no
/// console), so a forced stop falls back to ending the whole job, detached server included.
#[cfg(any(windows, test))]
fn attached_targets(own: u32, members: Option<&[u32]>, console: &[u32]) -> Option<Vec<u32>> {
    let members = members?;
    if console.is_empty() {
        return None;
    }
    Some(
        members
            .iter()
            .copied()
            .filter(|pid| *pid != own && console.contains(pid))
            .collect(),
    )
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
    /// Whether detached descendants may outlive the run.
    pub descendants: Descendants,
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

/// How long cleanup may take once the deadline passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupBudget {
    /// Wait after asking the owned tree to stop.
    pub graceful: Duration,
    /// Wait after forcing the owned tree to stop.
    pub forced: Duration,
    /// Wait for the pipes to close.
    pub drain: Duration,
}

impl CleanupBudget {
    /// The budget for tool runs and passthrough.
    pub const STANDARD: CleanupBudget = CleanupBudget {
        graceful: Duration::from_secs(2),
        forced: Duration::from_secs(2),
        drain: Duration::from_secs(5),
    };

    /// The whole cleanup bound.
    pub fn total(&self) -> Duration {
        self.graceful + self.forced + self.drain
    }
}

/// Run one child to a bounded outcome under the standard cleanup budget.
pub fn run(spec: &SpawnSpec) -> io::Result<ChildOutcome> {
    run_within(spec, CleanupBudget::STANDARD)
}

/// Run one child to a bounded outcome.
///
/// The child waits at most `timeout`. Then the owned tree is asked to stop
/// gracefully where the platform can (Unix), and is forced otherwise or
/// when it does not stop in time. Then the pipes are drained. The call returns no
/// later than the timeout plus the cleanup budget. Spawn and wait failures
/// are I/O errors. A tree that escapes its owner is reported as a timeout,
/// never as success.
pub fn run_within(spec: &SpawnSpec, cleanup: CleanupBudget) -> io::Result<ChildOutcome> {
    run_within_until(spec, cleanup, &|| None)
}

/// Like `run_within`, and also stops the child once `stop` returns a signal.
///
/// A stopped child is reported as `TimedOut`.
pub fn run_within_until(
    spec: &SpawnSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<ChildOutcome> {
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
    apply_env(&mut command, &spec.env);
    let mut owned = backend::spawn_owned(&mut command, spec.descendants)?;
    let limit = spec.capture.max_bytes;
    let stdout_rx = drain_pipe(owned.take_stdout(), limit);
    let stderr_rx = drain_pipe(owned.take_stderr(), limit);
    let start = Instant::now();
    let soft = start + spec.timeout;
    let mut hard = soft + cleanup.total();
    let mut status = None;
    let mut wait_error = None;
    while Instant::now() < soft {
        if stop().is_some() {
            hard = Instant::now() + cleanup.total();
            break;
        }
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
            let exit = poll_exit(&mut owned, hard.min(Instant::now() + cleanup.graceful));
            if exit.is_none() {
                backend::terminate_forced(&mut owned);
                let _ = poll_exit(&mut owned, hard.min(Instant::now() + cleanup.forced));
            }
            let _ = collect(&stdout_rx, hard);
            let _ = collect(&stderr_rx, hard);
            Ok(ChildOutcome::TimedOut)
        }
    }
}

fn apply_env(command: &mut Command, env: &EnvPolicy) {
    match env {
        EnvPolicy::Inherited { extra } => {
            command.envs(extra.iter().map(|(key, value)| (key, value)));
        }
        EnvPolicy::Controlled { vars } => {
            command.env_clear();
            command.envs(vars.iter().map(|(key, value)| (key, value)));
        }
    }
}

pub(crate) fn exit_of(status: &ExitStatus) -> Exit {
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

/// Where one of a streamed child's output streams goes.
pub enum StreamOutput {
    /// The child writes to the parent's stream directly.
    Inherit,
    /// The parent copies the child's stream into this sink.
    Forward(Box<dyn io::Write + Send>),
}

/// What a streamed child reads on stdin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStdin {
    /// The child reads the parent's stdin.
    Inherit,
    /// The child reads nothing.
    Null,
}

/// Who owns a streamed child's process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamGroup {
    /// The child runs in its own foreground group on the parent's terminal.
    Terminal,
    /// The child leads its own group, which the parent signals and cleans up.
    Owned,
}

/// One streamed child with no deadline.
pub struct StreamSpec {
    /// The binary followed by its arguments. Empty is rejected before spawning.
    pub argv: Vec<OsString>,
    /// The directory the child starts in.
    pub cwd: PathBuf,
    /// Inherited or controlled environment.
    pub env: EnvPolicy,
    /// What stdin the child reads.
    pub stdin: StreamStdin,
    /// Where stdout goes.
    pub stdout: StreamOutput,
    /// Where stderr goes.
    pub stderr: StreamOutput,
    /// Who owns the process tree.
    pub group: StreamGroup,
    /// Open files whose locks must last as long as the child tree.
    ///
    /// On Unix each handle is inherited by the child only (never by sibling
    /// spawns), so a `flock` lock on it stays held by the child and every
    /// descendant that keeps the descriptor, even after this process exits
    /// or crashes. A child that closes inherited descriptors (some daemons
    /// close everything above fd 2) drops that protection.
    ///
    /// On Windows a `LockFileEx` lock is not shared through inherited
    /// handles, so this process keeps the handles until the run ends and
    /// puts the child in a kill-on-close job: when the run ends, or this
    /// process dies, the whole tree ends with the lock.
    ///
    /// The handles are dropped only after the tree is settled.
    pub held: Vec<std::fs::File>,
}

/// How a streamed run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamOutcome {
    /// The child exited on its own.
    Exited(Exit),
    /// A stop signal arrived before the run completed.
    Cancelled { signal: i32 },
    /// The caller withdrew the run; its tree got SIGTERM and is gone.
    Withdrawn,
}

const WITHDRAW_SIGNAL: i32 = 15;

static FORWARD_TARGET: AtomicI32 = AtomicI32::new(0);

/// The stop signals dx forwards to a streamed child: SIGINT, SIGTERM and SIGHUP.
pub const STOP_SIGNALS: [i32; 3] = [2, 15, 1];

/// Records a stop signal and forwards it once to the running child's group; safe inside a signal handler.
pub fn forward_stop(signo: i32) {
    crate::record_interrupt(signo);
    forward(FORWARD_TARGET.load(Ordering::SeqCst), signo);
}

fn forward(target: i32, signo: i32) {
    backend::send(target, signo);
}

/// Whether this process is in the foreground group of its controlling terminal.
pub fn terminal_foreground() -> bool {
    backend::terminal_foreground()
}

/// The group a streamed child joins: the terminal foreground when this process holds it, else its own.
pub fn stream_group() -> StreamGroup {
    if terminal_foreground() {
        StreamGroup::Terminal
    } else {
        StreamGroup::Owned
    }
}

struct Forwarding {
    target: i32,
}

impl Forwarding {
    fn register(target: i32) -> Forwarding {
        if target != 0 {
            FORWARD_TARGET.store(target, Ordering::SeqCst);
        }
        Forwarding { target }
    }

    fn clear(&mut self) {
        if self.target != 0 {
            let _ =
                FORWARD_TARGET.compare_exchange(self.target, 0, Ordering::SeqCst, Ordering::SeqCst);
            self.target = 0;
        }
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Run one streamed child until it exits, or stop its owned tree once `stop` reports a signal.
pub fn run_streamed(
    spec: StreamSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<StreamOutcome> {
    run_streamed_until(spec, cleanup, stop, &|| false)
}

/// Like `run_streamed`, and also stops the tree with SIGTERM once `withdraw` returns true.
pub fn run_streamed_until(
    spec: StreamSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
    withdraw: &dyn Fn() -> bool,
) -> io::Result<StreamOutcome> {
    run_streamed_in(spec, cleanup, stop, withdraw, crate::groups::lease_dir())
}

/// Like `run_streamed_until`, with owned group leases kept in `lease_dir`.
pub(crate) fn run_streamed_in(
    spec: StreamSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
    withdraw: &dyn Fn() -> bool,
    lease_dir: Option<&Path>,
) -> io::Result<StreamOutcome> {
    if let Some(signal) = stop() {
        return Ok(StreamOutcome::Cancelled { signal });
    }
    if withdraw() {
        return Ok(StreamOutcome::Withdrawn);
    }
    let (program, args) = spec
        .argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(&spec.cwd)
        .stdin(match spec.stdin {
            StreamStdin::Inherit => Stdio::inherit(),
            StreamStdin::Null => Stdio::null(),
        })
        .stdout(output_stdio(&spec.stdout))
        .stderr(output_stdio(&spec.stderr));
    apply_env(&mut command, &spec.env);
    let owned = spec.group == StreamGroup::Owned;
    let mut lease = lease_dir
        .filter(|_| owned)
        .and_then(crate::groups::GroupLease::acquire_in);
    let held = spec.held;
    let mut job = if owned {
        None
    } else {
        backend::TerminalJob::start()
    };
    let group = match (&job, owned) {
        (_, true) => backend::Group::Owned,
        (Some(job), false) => backend::Group::Join(job.pgid()),
        (None, false) => backend::Group::Own,
    };
    let (mut child, tree) = backend::spawn_streamed(&mut command, group, &held)?;
    if let Some(lease) = lease.as_mut() {
        lease.record(child.id() as i32);
    }
    let target = backend::stream_target(&child, group);
    let mut forwarding = Forwarding::register(target);
    if let Some(job) = job.as_ref() {
        record_reported(job);
    }
    let mut stopped = stop();
    if let Some(signal) = stopped {
        backend::send(target, signal);
    }
    let mut withdrawn = false;
    let mut drained = Vec::new();
    if let (StreamOutput::Forward(sink), Some(stdout)) = (spec.stdout, child.stdout.take()) {
        drained.push(copy_out(stdout, sink));
    }
    if let (StreamOutput::Forward(sink), Some(stderr)) = (spec.stderr, child.stderr.take()) {
        drained.push(copy_out(stderr, sink));
    }
    let mut escalate_at = stopped
        .filter(|_| owned)
        .map(|_| Instant::now() + cleanup.graceful);
    let mut abandon_at = None;
    let waited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(err) => {
                backend::force_stream(&mut child, target);
                drop(job);
                return Err(err);
            }
        }
        if let Some(job) = job.as_ref() {
            record_reported(job);
            job.tend(child.id() as i32);
        }
        if stopped.is_none() {
            stopped = stop();
            if stopped.is_some() && owned && escalate_at.is_none() && abandon_at.is_none() {
                escalate_at = Some(Instant::now() + cleanup.graceful);
            }
        }
        if stopped.is_none() && !withdrawn && withdraw() {
            withdrawn = true;
            backend::send(target, WITHDRAW_SIGNAL);
            escalate_at = Some(Instant::now() + cleanup.graceful);
        }
        let now = Instant::now();
        if escalate_at.is_some_and(|at| now >= at) {
            backend::force_stream(&mut child, target);
            escalate_at = None;
            abandon_at = Some(now + cleanup.forced);
        }
        if abandon_at.is_some_and(|at| now >= at) {
            break None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if !owned {
        forwarding.clear();
        if let Some(job) = job.take() {
            record_reported(&job);
        }
    }
    let deadline = Instant::now() + cleanup.drain;
    while !drained.is_empty() {
        if stopped.is_none() {
            stopped = stop();
        }
        if stopped.is_some() || withdrawn || Instant::now() >= deadline {
            break;
        }
        await_drained(&mut drained, Instant::now() + POLL_INTERVAL);
    }
    if stopped.is_none() {
        stopped = stop();
    }
    let settle = if stopped.is_some() || withdrawn {
        true
    } else if owned && backend::tree_alive(target) {
        backend::send(target, WITHDRAW_SIGNAL);
        true
    } else {
        false
    };
    if settle {
        if owned {
            settle_owned_tree(target, cleanup);
        }
        let until = Instant::now() + cleanup.forced;
        while !drained.is_empty() && Instant::now() < until {
            await_drained(&mut drained, until);
        }
    }
    drop(forwarding);
    drop(lease);
    // Only the Windows tree owns a job; the drop order matters there.
    #[allow(clippy::drop_non_drop)]
    drop(tree);
    drop(held);
    Ok(match (stopped, waited) {
        (Some(signal), _) => StreamOutcome::Cancelled { signal },
        (None, Some(status)) if !withdrawn => StreamOutcome::Exited(exit_of(&status)),
        (None, _) => StreamOutcome::Withdrawn,
    })
}

/// Records the stop signals the terminal delivered straight to the child's group; they are not forwarded again.
fn record_reported(job: &backend::TerminalJob) {
    while let Some(signo) = job.take_reported() {
        crate::record_interrupt(signo);
    }
}

fn settle_owned_tree(target: i32, cleanup: CleanupBudget) {
    if !wait_tree_gone(target, Instant::now() + cleanup.graceful) {
        backend::send_kill(target);
        let _ = wait_tree_gone(target, Instant::now() + cleanup.forced);
    }
}

fn wait_tree_gone(target: i32, until: Instant) -> bool {
    loop {
        if !backend::tree_alive(target) {
            return true;
        }
        let now = Instant::now();
        if now >= until {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL.min(until - now));
    }
}

fn output_stdio(output: &StreamOutput) -> Stdio {
    match output {
        StreamOutput::Inherit => Stdio::inherit(),
        StreamOutput::Forward(_) => Stdio::piped(),
    }
}

fn await_drained(pending: &mut Vec<mpsc::Receiver<()>>, until: Instant) {
    let wait = until.saturating_duration_since(Instant::now());
    if let Some(first) = pending.first() {
        if !matches!(
            first.recv_timeout(wait),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            pending.remove(0);
        }
    }
    pending.retain(|rx| matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

fn copy_out<R: io::Read + Send + 'static>(
    mut pipe: R,
    mut sink: Box<dyn io::Write + Send>,
) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        backend::allow_background_tty_writes();
        let _ = io::copy(&mut pipe, &mut sink);
        let _ = sink.flush();
        drop(pipe);
        let _ = tx.send(());
    });
    rx
}

#[cfg(unix)]
#[path = "terminal.rs"]
mod terminal;

mod backend {
    /// Which process group a streamed child joins.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(not(unix), allow(dead_code))]
    pub enum Group {
        /// It leads its own group, which the parent owns and cleans up.
        Owned,
        /// It joins this terminal job's foreground group.
        Join(i32),
        /// It leads its own group and nobody cleans up after it.
        Own,
    }

    #[cfg(unix)]
    mod unix {
        use super::super::{Command, Descendants, ExitStatus};
        use super::Group;
        use std::io;
        use std::process::{Child, ChildStderr, ChildStdout};

        const SIGTERM: i32 = 15;
        const SIGKILL: i32 = 9;

        unsafe extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
            fn getpgrp() -> i32;
            fn tcgetpgrp(fd: i32) -> i32;
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

        #[cfg(target_os = "linux")]
        fn stop_with_parent(command: &mut Command) {
            use std::os::unix::process::CommandExt;
            const PR_SET_PDEATHSIG: i32 = 1;
            unsafe extern "C" {
                fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
                fn getppid() -> i32;
            }
            let parent = std::process::id() as i32;
            unsafe {
                command.pre_exec(move || {
                    prctl(PR_SET_PDEATHSIG, SIGTERM as u64, 0, 0, 0);
                    if getppid() != parent {
                        return Err(io::Error::from(io::ErrorKind::BrokenPipe));
                    }
                    Ok(())
                });
            }
        }

        #[cfg(not(target_os = "linux"))]
        fn stop_with_parent(_command: &mut Command) {}

        pub fn spawn_owned(
            command: &mut Command,
            _descendants: Descendants,
        ) -> io::Result<OwnedChild> {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            stop_with_parent(command);
            Ok(OwnedChild {
                child: command.spawn()?,
            })
        }

        fn signal_group(owned: &mut OwnedChild, signo: i32) {
            let pgid = -(owned.child.id() as i32);
            unsafe {
                kill(pgid, signo);
            }
        }

        pub fn terminate_graceful(owned: &mut OwnedChild) {
            signal_group(owned, SIGTERM);
        }

        pub fn terminate_forced(owned: &mut OwnedChild) {
            signal_group(owned, SIGKILL);
        }

        pub fn termination_signal(status: &ExitStatus) -> Option<i32> {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        }

        /// Nothing beyond the inherited descriptors keeps a Unix tree's locks.
        pub struct StreamTree;

        const F_GETFD: i32 = 1;
        const F_SETFD: i32 = 2;
        const FD_CLOEXEC: i32 = 1;

        unsafe extern "C" {
            fn fcntl(fd: i32, cmd: i32, ...) -> i32;
        }

        /// Clears close-on-exec on the held descriptors in the child only,
        /// so concurrent spawns from other threads never inherit them.
        fn inherit_held(command: &mut Command, held: &[std::fs::File]) {
            use std::os::fd::AsRawFd;
            use std::os::unix::process::CommandExt;
            if held.is_empty() {
                return;
            }
            let fds: Vec<i32> = held.iter().map(AsRawFd::as_raw_fd).collect();
            // SAFETY: the hook only calls fcntl on descriptors collected before
            // the fork; it allocates nothing and takes no locks.
            unsafe {
                command.pre_exec(move || {
                    for &fd in &fds {
                        let flags = fcntl(fd, F_GETFD);
                        if flags < 0 || fcntl(fd, F_SETFD, flags & !FD_CLOEXEC) < 0 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                });
            }
        }

        pub fn spawn_streamed(
            command: &mut Command,
            group: Group,
            held: &[std::fs::File],
        ) -> io::Result<(Child, StreamTree)> {
            use std::os::unix::process::CommandExt;
            match group {
                Group::Owned => {
                    command.process_group(0);
                    stop_with_parent(command);
                }
                Group::Join(pgid) => {
                    command.process_group(pgid);
                }
                Group::Own => {
                    command.process_group(0);
                }
            }
            inherit_held(command, held);
            Ok((command.spawn()?, StreamTree))
        }

        /// The negative group id every forwarded signal goes to.
        pub fn stream_target(child: &Child, group: Group) -> i32 {
            match group {
                Group::Join(pgid) => -pgid,
                Group::Owned | Group::Own => -(child.id() as i32),
            }
        }

        pub fn send(target: i32, signo: i32) {
            if target != 0 {
                unsafe {
                    kill(target, signo);
                }
            }
        }

        pub fn send_kill(target: i32) {
            send(target, SIGKILL);
        }

        pub fn force_stream(_child: &mut Child, target: i32) {
            send_kill(target);
        }

        pub fn tree_alive(target: i32) -> bool {
            target != 0 && unsafe { kill(target, 0) } == 0
        }

        pub fn terminal_foreground() -> bool {
            use std::os::fd::AsRawFd;
            let Ok(tty) = std::fs::File::open("/dev/tty") else {
                return false;
            };
            let foreground = unsafe { tcgetpgrp(tty.as_raw_fd()) };
            foreground > 0 && foreground == unsafe { getpgrp() }
        }
    }

    #[cfg(windows)]
    mod windows {
        use super::super::{Command, Descendants, ExitStatus};
        use std::ffi::c_void;
        use std::io;
        use std::os::windows::io::AsRawHandle;
        use std::process::{ChildStderr, ChildStdout};

        type Handle = *mut c_void;

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
            fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
            fn SetInformationJobObject(
                handle: Handle,
                class: i32,
                info: *const c_void,
                len: u32,
            ) -> i32;
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
                return Err(io::Error::last_os_error());
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
                        failure = Some(io::Error::last_os_error());
                        break;
                    }
                    let previous = unsafe { ResumeThread(thread) };
                    let err = io::Error::last_os_error();
                    unsafe {
                        CloseHandle(thread);
                    }
                    if previous == u32::MAX {
                        failure = Some(err);
                        break;
                    }
                    resumed += 1;
                }
                more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
            }
            unsafe {
                CloseHandle(snapshot);
            }
            match failure {
                Some(err) => Err(err),
                None if resumed == 0 => Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "the suspended child has no thread",
                )),
                None => Ok(()),
            }
        }

        fn console_members() -> Vec<u32> {
            let mut list = vec![0u32; 64];
            for _ in 0..LIST_ATTEMPTS {
                let count =
                    unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) } as usize;
                if count <= list.len() {
                    list.truncate(count);
                    return list;
                }
                list.resize(count + 16, 0);
            }
            Vec::new()
        }

        struct Job {
            handle: Handle,
            descendants: Descendants,
        }

        impl Job {
            fn create(descendants: Descendants) -> io::Result<Job> {
                let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
                if handle.is_null() {
                    return Err(io::Error::last_os_error());
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
                    let err = io::Error::last_os_error();
                    unsafe {
                        CloseHandle(handle);
                    }
                    return Err(err);
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
                    }
                    if !queried && err.raw_os_error() != Some(ERROR_MORE_DATA) {
                        return None;
                    }
                    capacity = (assigned + 16).max(capacity * 2);
                }
                None
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
                    return;
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
                let Some(targets) = super::super::attached_targets(
                    std::process::id(),
                    members.as_deref(),
                    &console,
                ) else {
                    return false;
                };
                for pid in targets {
                    self.kill_member(pid);
                }
                true
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

        pub fn spawn_owned(
            command: &mut Command,
            descendants: Descendants,
        ) -> io::Result<OwnedChild> {
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
                    other => other?,
                },
            };
            let started = job.assign(&child).and_then(|()| resume_threads(child.id()));
            if let Err(err) = started {
                job.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
            Ok(OwnedChild { child, job })
        }

        /// Keeps a streamed child's kill-on-close job open while the run lasts.
        pub struct StreamTree {
            _job: Option<Job>,
        }

        /// Spawns a streamed child; with held handles the child runs in a
        /// kill-on-close job so the tree never outlives the handles' locks.
        pub fn spawn_streamed(
            command: &mut Command,
            _group: super::Group,
            held: &[std::fs::File],
        ) -> io::Result<(std::process::Child, StreamTree)> {
            use std::os::windows::process::CommandExt;
            if held.is_empty() {
                return Ok((command.spawn()?, StreamTree { _job: None }));
            }
            let job = Job::create(Descendants::Owned)?;
            let mut child = command.creation_flags(CREATE_SUSPENDED).spawn()?;
            let started = job.assign(&child).and_then(|()| resume_threads(child.id()));
            if let Err(err) = started {
                job.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
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
    }

    #[cfg(unix)]
    pub use super::terminal::{allow_background_tty_writes, TerminalJob};

    #[cfg(unix)]
    pub use unix::{
        force_stream, send, send_kill, spawn_owned, spawn_streamed, stream_target,
        terminal_foreground, terminate_forced, terminate_graceful, termination_signal, tree_alive,
        OwnedChild,
    };

    #[cfg(not(unix))]
    mod plain_stream {
        #[cfg(not(windows))]
        use super::super::Command;
        #[cfg(not(windows))]
        use std::io;
        use std::process::Child;

        /// The parent's own handles are all that keep a portable tree's locks.
        #[cfg(not(windows))]
        pub struct StreamTree;

        #[cfg(not(windows))]
        pub fn spawn_streamed(
            command: &mut Command,
            _group: super::Group,
            _held: &[std::fs::File],
        ) -> io::Result<(Child, StreamTree)> {
            Ok((command.spawn()?, StreamTree))
        }

        pub fn stream_target(_child: &Child, _group: super::Group) -> i32 {
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

        /// No portable terminal job; streamed children share the console.
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
    }

    #[cfg(not(any(unix, windows)))]
    pub use plain_stream::spawn_streamed;
    #[cfg(not(unix))]
    pub use plain_stream::{
        allow_background_tty_writes, force_stream, send, send_kill, stream_target,
        terminal_foreground, tree_alive, TerminalJob,
    };
    #[cfg(windows)]
    pub use windows::{
        spawn_owned, spawn_streamed, terminate_forced, terminate_graceful, termination_signal,
        OwnedChild,
    };

    #[cfg(not(any(unix, windows)))]
    mod portable {
        use super::super::{Command, Descendants, ExitStatus};
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

        pub fn spawn_owned(
            command: &mut Command,
            _descendants: Descendants,
        ) -> io::Result<OwnedChild> {
            Ok(OwnedChild {
                child: command.spawn()?,
            })
        }

        pub fn terminate_graceful(owned: &mut OwnedChild) {
            terminate_forced(owned);
        }

        pub fn terminate_forced(owned: &mut OwnedChild) {
            let _ = owned.child.kill();
        }

        pub fn termination_signal(_status: &ExitStatus) -> Option<i32> {
            None
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub use portable::{
        spawn_owned, terminate_forced, terminate_graceful, termination_signal, OwnedChild,
    };
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
