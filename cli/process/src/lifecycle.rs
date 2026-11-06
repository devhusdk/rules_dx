//! Synchronous child lifecycle shared below domain adapters.

use std::ffi::OsString;
use std::io;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
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
#[cfg_attr(not(windows), allow(dead_code))]
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
    run_command_until(
        &mut command,
        &spec.capture,
        spec.timeout,
        spec.descendants,
        cleanup,
        stop,
    )
}

/// Like `run_within_until`, for a `Command` the caller has already set up
/// (program, args, cwd, env). Stdin is set to null and stdout and stderr are
/// piped here. A stopped child is reported as `TimedOut`.
pub fn run_command_until(
    command: &mut Command,
    capture: &CapturePolicy,
    timeout: Duration,
    descendants: Descendants,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<ChildOutcome> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owned = backend::spawn_owned(command, descendants)?;
    let limit = capture.max_bytes;
    let stdout_rx = drain_pipe(owned.take_stdout(), limit);
    let stderr_rx = drain_pipe(owned.take_stderr(), limit);
    let start = Instant::now();
    let soft = start + timeout;
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
            // LCOV_EXCL_START - reason: try_wait failing on a live child cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            Err(err) => {
                wait_error = Some(err);
                break;
            } // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
    }
    // LCOV_EXCL_START - reason: try_wait failing on a live child cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    if let Some(err) = wait_error {
        backend::terminate_forced(&mut owned);
        let _ = poll_exit(&mut owned, hard);
        return Err(err);
    }
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    match status {
        Some(exit) => {
            backend::end_owned_descendants(&mut owned);
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
            backend::terminate_forced(&mut owned);
            if exit.is_none() {
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
            Err(_) => return None, // LCOV_EXCL_LINE - reason: try_wait failing on a live child cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
        Err(mpsc::RecvTimeoutError::Disconnected) => Drain::Panicked, // LCOV_EXCL_LINE - reason: the drain thread cannot panic, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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

/// How many streamed runs can have stop signals forwarded to them at once.
/// A run that finds the table full is not forwarded to.
const FORWARD_SLOTS: usize = 8;

struct Slot {
    target: AtomicI32,
    claimed: AtomicBool,
}

impl Slot {
    const fn new() -> Slot {
        Slot {
            target: AtomicI32::new(0),
            claimed: AtomicBool::new(false),
        }
    }
}

static FORWARDS: [Slot; FORWARD_SLOTS] = [const { Slot::new() }; FORWARD_SLOTS];
static REGISTER: Mutex<()> = Mutex::new(());

/// The stop signals dx forwards to a streamed child: SIGINT, SIGTERM and SIGHUP.
pub const STOP_SIGNALS: [i32; 3] = [2, 15, 1];

/// Makes Ctrl-C and Ctrl-Break record a stop like SIGINT.
#[cfg(windows)]
pub use backend::install_console_stop;

/// Maps a Windows console control event to the signal it acts as: Ctrl-C and Ctrl-Break are SIGINT.
pub fn console_stop_signal(ctrl_type: u32) -> Option<i32> {
    match ctrl_type {
        0 | 1 => Some(2),
        _ => None,
    }
}

/// Records a stop signal and forwards it once to each running child's group; safe inside a signal handler.
// LCOV_EXCL_START - reason: the signal handler entry records a process-wide interrupt, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
pub fn forward_stop(signo: i32) {
    forward_from(&FORWARDS, signo, crate::record_interrupt);
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

fn forward_from(slots: &[Slot], signo: i32, record: fn(i32)) {
    let mut targets = [0; FORWARD_SLOTS];
    for (copy, slot) in targets.iter_mut().zip(slots) {
        let target = slot.target.load(Ordering::SeqCst);
        if target != 0 {
            slot.claimed.store(true, Ordering::SeqCst);
            *copy = target;
        }
    }
    record(signo);
    for target in targets {
        if target != 0 {
            forward(target, signo);
        }
    }
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
        StreamGroup::Terminal // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    } else {
        StreamGroup::Owned
    }
}

/// The only writer to `FORWARDS`.
struct Forwarding {
    target: i32,
    slot: Option<&'static Slot>,
}

impl Forwarding {
    fn register(target: i32) -> Forwarding {
        Self::register_in(&FORWARDS, target)
    }

    fn register_in(slots: &'static [Slot], target: i32) -> Forwarding {
        let mut slot = None;
        if target != 0 {
            let _guard = REGISTER
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for candidate in slots {
                if candidate.target.load(Ordering::SeqCst) == 0 {
                    candidate.claimed.store(false, Ordering::SeqCst);
                    candidate.target.store(target, Ordering::SeqCst);
                    slot = Some(candidate);
                    break;
                }
            }
        }
        Forwarding { target, slot }
    }

    /// Whether the signal handler already sent a stop signal to this run.
    fn claimed(&self) -> bool {
        self.slot
            .is_some_and(|slot| slot.claimed.load(Ordering::SeqCst))
    }

    fn clear(&mut self) {
        if let Some(slot) = self.slot.take() {
            let _ =
                slot.target
                    .compare_exchange(self.target, 0, Ordering::SeqCst, Ordering::SeqCst);
        }
        self.target = 0;
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        self.clear();
    }
}

/// The stdin a streamed child gets. A child alone in a background group would be stopped by
/// SIGTTIN on a terminal it does not own, so it reads nothing there.
fn stream_stdin(requested: StreamStdin, own_group: bool, stdin_tty: bool) -> StreamStdin {
    if requested == StreamStdin::Inherit && own_group && stdin_tty {
        StreamStdin::Null
    } else {
        requested
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
        (Some(job), false) => backend::Group::Join(job.pgid()), // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        (None, false) => backend::Group::Own,
    };
    let own_group = matches!(group, backend::Group::Own);
    command.stdin(
        match stream_stdin(spec.stdin, own_group, std::io::stdin().is_terminal()) {
            StreamStdin::Inherit => Stdio::inherit(),
            StreamStdin::Null => Stdio::null(),
        },
    );
    let (mut child, tree) = backend::spawn_streamed(&mut command, group, &held)?;
    if let Some(lease) = lease.as_mut() {
        lease.record(child.id() as i32);
    }
    let target = backend::stream_target(&child, group);
    let mut forwarding = Forwarding::register(target);
    if let Some(job) = job.as_ref() {
        record_reported(job); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    let mut stopped = stop();
    if let Some(signal) = stopped.filter(|_| !forwarding.claimed()) {
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
            // LCOV_EXCL_START - reason: try_wait failing on a live child cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            Err(err) => {
                backend::force_stream(&mut child, target);
                #[allow(clippy::drop_non_drop)]
                drop(job);
                return Err(err);
            } // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        if let Some(job) = job.as_ref() {
            record_reported(job); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            job.tend(child.id() as i32); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
            break None; // LCOV_EXCL_LINE - reason: a child that survives SIGKILL cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if !owned {
        forwarding.clear();
        if let Some(job) = job.take() {
            record_reported(&job); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
// LCOV_EXCL_START - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
fn record_reported(job: &backend::TerminalJob) {
    while let Some(signo) = job.take_reported() {
        crate::record_interrupt(signo);
    }
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

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
    if let Some(first) = pending.first()
        && !matches!(
            first.recv_timeout(wait),
            Err(mpsc::RecvTimeoutError::Timeout)
        )
    {
        pending.remove(0);
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

#[cfg(unix)]
#[path = "lifecycle_unix.rs"]
mod unix_backend;

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
    use super::unix_backend as unix;

    #[cfg(windows)]
    mod windows {
        use super::super::{Command, Descendants, ExitStatus};
        use std::ffi::c_void;
        use std::io;
        use std::os::windows::io::AsRawHandle;
        use std::process::{ChildStderr, ChildStdout};

        type Handle = *mut c_void;

        unsafe extern "system" fn console_stop(ctrl_type: u32) -> i32 {
            match super::super::console_stop_signal(ctrl_type) {
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

        /// Spawns a streamed child; an owned or lock-holding child runs in a
        /// kill-on-close job so its tree never outlives the run.
        pub fn spawn_streamed(
            command: &mut Command,
            group: super::Group,
            held: &[std::fs::File],
        ) -> io::Result<(std::process::Child, StreamTree)> {
            use std::os::windows::process::CommandExt;
            if held.is_empty() && !matches!(group, super::Group::Owned) {
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
    pub use super::terminal::{TerminalJob, allow_background_tty_writes};

    #[cfg(unix)]
    pub use unix::{
        OwnedChild, end_owned_descendants, force_stream, send, send_kill, spawn_owned,
        spawn_streamed, stream_target, terminal_foreground, terminate_forced, terminate_graceful,
        termination_signal, tree_alive,
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
        TerminalJob, allow_background_tty_writes, force_stream, send, send_kill, stream_target,
        terminal_foreground, tree_alive,
    };
    #[cfg(not(unix))]
    pub fn end_owned_descendants(_owned: &mut OwnedChild) {}

    #[cfg(windows)]
    pub use windows::{
        OwnedChild, install_console_stop, spawn_owned, spawn_streamed, terminate_forced,
        terminate_graceful, termination_signal,
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
        OwnedChild, spawn_owned, terminate_forced, terminate_graceful, termination_signal,
    };
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
