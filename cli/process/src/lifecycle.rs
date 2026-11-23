use dx_os_sys::console::CtrlEvent;
use std::ffi::OsString;
use std::io;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvPolicy {
    Inherited { extra: Vec<(OsString, OsString)> },
    Controlled { vars: Vec<(OsString, OsString)> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePolicy {
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descendants {
    Owned,
    Detachable,
    Foreground,
}

#[cfg(windows)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: EnvPolicy,
    pub capture: CapturePolicy,
    pub timeout: Duration,
    pub descendants: Descendants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

impl Exit {
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        match self {
            Exit::Code(code) => Some(*code),
            Exit::Signal(_) => None,
        }
    }
}

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

fn after(at: Instant, span: Duration) -> Instant {
    const CENTURY: Duration = Duration::from_secs(100 * 365 * 24 * 3600);
    at.checked_add(span)
        .or_else(|| at.checked_add(CENTURY))
        .unwrap_or(at)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupBudget {
    pub graceful: Duration,
    pub forced: Duration,
    pub drain: Duration,
}

impl CleanupBudget {
    pub const STANDARD: CleanupBudget = CleanupBudget {
        graceful: Duration::from_secs(2),
        forced: Duration::from_secs(2),
        drain: Duration::from_secs(5),
    };

    #[must_use]
    pub fn total(&self) -> Duration {
        self.graceful
            .saturating_add(self.forced)
            .saturating_add(self.drain)
    }
}

pub fn run(spec: &SpawnSpec) -> io::Result<ChildOutcome> {
    run_within(spec, CleanupBudget::STANDARD)
}

pub fn run_within(spec: &SpawnSpec, cleanup: CleanupBudget) -> io::Result<ChildOutcome> {
    run_within_until(spec, cleanup, &|| None)
}

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

pub fn run_command_until(
    command: &mut Command,
    capture: &CapturePolicy,
    timeout: Duration,
    descendants: Descendants,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<ChildOutcome> {
    run_command_with_input(command, None, capture, timeout, descendants, cleanup, stop)
}

pub fn run_command_with_input(
    command: &mut Command,
    input: Option<&[u8]>,
    capture: &CapturePolicy,
    timeout: Duration,
    descendants: Descendants,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<ChildOutcome> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::apply_bazel_env(command);
    let mut owned = backend::spawn_owned(command, descendants)?;
    if let (Some(input), Some(mut stdin)) = (input, owned.take_stdin()) {
        let input = input.to_vec();
        std::thread::spawn(move || {
            use std::io::Write as _;
            drop(stdin.write_all(&input));
        });
    }
    let limit = capture.max_bytes;
    let over = Arc::new(AtomicBool::new(false));
    let stdout_rx = drain_pipe(owned.take_stdout(), limit, Arc::clone(&over));
    let stderr_rx = drain_pipe(owned.take_stderr(), limit, Arc::clone(&over));
    let start = Instant::now();
    let soft = after(start, timeout);
    let mut hard = after(soft, cleanup.total());
    let mut status = None;
    let mut wait_error = None;
    while Instant::now() < soft {
        if stop().is_some() || over.load(Ordering::SeqCst) {
            hard = after(Instant::now(), cleanup.total());
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
        let _: Option<ExitStatus> = poll_exit(&mut owned, hard);
        return Err(err);
    }
    // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    if let Some(exit) = status {
        backend::end_owned_descendants(&mut owned);
        let stdout = collect(&stdout_rx, hard);
        let stderr = collect(&stderr_rx, hard);
        if let (Drain::Ready(stdout), Drain::Ready(stderr)) = (stdout, stderr) {
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
        } else {
            backend::terminate_forced(&mut owned);
            Ok(ChildOutcome::TimedOut)
        }
    } else {
        backend::terminate_graceful(&mut owned);
        let _: Option<ExitStatus> = poll_exit(
            &mut owned,
            hard.min(after(Instant::now(), cleanup.graceful)),
        );
        backend::terminate_forced(&mut owned);
        let _: Option<ExitStatus> =
            poll_exit(&mut owned, hard.min(after(Instant::now(), cleanup.forced)));
        let _: Drain = collect(&stdout_rx, hard);
        let _: Drain = collect(&stderr_rx, hard);
        if over.load(Ordering::SeqCst) {
            Ok(ChildOutcome::OutputTooLarge { limit })
        } else {
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
    exit_from(status.code(), backend::termination_signal(status))
}

fn exit_from(code: Option<i32>, signal: Option<i32>) -> Exit {
    match code {
        Some(code) => Exit::Code(code),
        None => Exit::Signal(signal.unwrap_or(0)),
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
    over: Arc<AtomicBool>,
) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    match pipe {
        None => {
            drop(tx.send(Vec::new()));
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
                            buf.extend(chunk.iter().take(n.min(room)));
                            if buf.len() > limit {
                                over.store(true, Ordering::SeqCst);
                            }
                        }
                        Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                drop(tx.send(buf));
            });
        }
    }
    rx
}

pub enum StreamOutput {
    Inherit,
    Forward(Box<dyn io::Write + Send>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamStdin {
    Inherit,
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamGroup {
    Terminal,
    Owned,
}

pub struct StreamSpec {
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: EnvPolicy,
    pub stdin: StreamStdin,
    pub stdout: StreamOutput,
    pub stderr: StreamOutput,
    pub group: StreamGroup,
    pub held: Vec<std::fs::File>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOutcome {
    Exited(Exit),
    Cancelled { signal: i32 },
    Withdrawn,
}

const WITHDRAW_SIGNAL: i32 = 15;

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

pub use dx_os_sys::STOP_SIGNALS;

#[cfg(windows)]
pub use backend::install_console_stop;

#[must_use]
pub fn console_stop_signal(event: CtrlEvent) -> Option<i32> {
    match event {
        CtrlEvent::C | CtrlEvent::Break => Some(2),
        CtrlEvent::Close | CtrlEvent::Logoff | CtrlEvent::Shutdown => None,
    }
}

// LCOV_EXCL_START - reason: the signal handler entry records a process-wide interrupt, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
pub fn forward_stop(signo: i32) {
    forward_from(&FORWARDS, signo, crate::record_interrupt, backend::send);
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

fn forward_from(slots: &[Slot], signo: i32, record: fn(i32), send: fn(i32, i32)) {
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
            send(target, signo);
        }
    }
}

#[must_use]
pub fn terminal_foreground() -> bool {
    backend::terminal_foreground()
}

#[must_use]
pub fn stream_group() -> StreamGroup {
    stream_group_for(terminal_foreground())
}

fn stream_group_for(foreground: bool) -> StreamGroup {
    if foreground {
        StreamGroup::Terminal
    } else {
        StreamGroup::Owned
    }
}

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
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    fn claimed(&self) -> bool {
        self.slot
            .is_some_and(|slot| slot.claimed.load(Ordering::SeqCst))
    }

    fn clear(&mut self) {
        if let Some(slot) = self.slot.take() {
            slot.target
                .compare_exchange(self.target, 0, Ordering::SeqCst, Ordering::SeqCst)
                .unwrap_or_else(|current| current);
        }
        self.target = 0;
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        self.clear();
    }
}

fn stream_stdin(requested: StreamStdin, own_group: bool, stdin_tty: bool) -> StreamStdin {
    if requested == StreamStdin::Inherit && own_group && stdin_tty {
        StreamStdin::Null
    } else {
        requested
    }
}

pub fn run_streamed(
    spec: StreamSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
) -> io::Result<StreamOutcome> {
    run_streamed_until(spec, cleanup, stop, &|| false)
}

pub fn run_streamed_until(
    spec: StreamSpec,
    cleanup: CleanupBudget,
    stop: &dyn Fn() -> Option<i32>,
    withdraw: &dyn Fn() -> bool,
) -> io::Result<StreamOutcome> {
    run_streamed_in(spec, cleanup, stop, withdraw, crate::groups::lease_dir())
}

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
    crate::apply_bazel_env(&mut command);
    let owned = spec.group == StreamGroup::Owned;
    let lease = lease_dir
        .filter(|_| owned)
        .and_then(crate::groups::GroupLease::acquire_in);
    let held = spec.held;
    let mut job = if owned {
        None
    } else {
        backend::TerminalJob::start()
    };
    let terminal = job.is_some();
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Some(lease) = lease.as_ref() {
        lease.record_from_child(&mut command);
    }
    let (mut child, tree) = backend::spawn_streamed(&mut command, group, &held)?;
    drop(command);
    if let Some(lease) = lease.as_ref()
        && let Err(error) = lease.record_unwritten(child.id().cast_signed())
    {
        dx_error::stdio::eprint_line(format_args!(
            "dx: cannot record process group lease {}: {error}",
            lease.path().display()
        ));
    }
    let target = backend::stream_target(&child, group);
    let mut forwarding = Forwarding::register(target);
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
        .map(|_| after(Instant::now(), cleanup.graceful));
    let mut abandon_at = None;
    let waited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            // LCOV_EXCL_START - reason: try_wait failing on a live child cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            Err(err) => {
                backend::force_stream(&mut child, target);
                #[cfg_attr(
                    windows,
                    expect(
                        clippy::drop_non_drop,
                        reason = "the Windows terminal job owns nothing"
                    )
                )]
                drop(job);
                return Err(err);
            } // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        if let Some(job) = job.as_ref() {
            record_reported(job); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            job.tend(child.id().cast_signed()); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        if stopped.is_none() {
            stopped = stop();
            if stopped.is_some() && owned && escalate_at.is_none() && abandon_at.is_none() {
                escalate_at = Some(after(Instant::now(), cleanup.graceful));
            }
        }
        if stopped.is_none() && !withdrawn && withdraw() {
            withdrawn = true;
            backend::send(target, WITHDRAW_SIGNAL);
            escalate_at = Some(after(Instant::now(), cleanup.graceful));
        }
        let now = Instant::now();
        if escalate_at.is_some_and(|at| now >= at) {
            backend::force_stream(&mut child, target);
            escalate_at = None;
            abandon_at = Some(after(now, cleanup.forced));
        }
        if abandon_at.is_some_and(|at| now >= at) {
            break None; // LCOV_EXCL_LINE - reason: a child that survives SIGKILL cannot be provoked, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if !owned {
        forwarding.clear();
        if let Some(job) = job.take() {
            settle_reported(&job); // LCOV_EXCL_LINE - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
    }
    if stopped.is_none()
        && let Some(signo) = group_stop(terminal, waited.as_ref().map(exit_of))
    {
        // LCOV_EXCL_START - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        crate::record_interrupt(signo);
        stopped = Some(signo);
    } // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    let deadline = after(Instant::now(), cleanup.drain);
    while !drained.is_empty() {
        if stopped.is_none() {
            stopped = stop();
        }
        if stopped.is_some() || withdrawn || Instant::now() >= deadline {
            break;
        }
        await_drained(&mut drained, after(Instant::now(), POLL_INTERVAL));
    }
    if stopped.is_none() {
        stopped = stop();
    }
    let settle = stopped.is_some() || withdrawn || (owned && withdraw_leftover(&OS_TREE, target));
    if settle {
        if owned {
            settle_owned_tree(&OS_TREE, target, cleanup);
        }
        let until = after(Instant::now(), cleanup.forced);
        while !drained.is_empty() && Instant::now() < until {
            await_drained(&mut drained, until);
        }
    }
    drop(forwarding);
    drop(lease);
    // Only the Windows tree owns a job; the drop order matters there.
    #[cfg_attr(
        not(windows),
        expect(clippy::drop_non_drop, reason = "only the Windows tree owns a job")
    )]
    drop(tree);
    drop(held);
    Ok(match (stopped, waited) {
        (Some(signal), _) => StreamOutcome::Cancelled { signal },
        (None, Some(status)) if !withdrawn => StreamOutcome::Exited(exit_of(&status)),
        (None, _) => StreamOutcome::Withdrawn,
    })
}

fn group_stop(terminal: bool, exit: Option<Exit>) -> Option<i32> {
    match exit {
        Some(Exit::Signal(signo)) if terminal && STOP_SIGNALS.contains(&signo) => Some(signo),
        _ => None,
    }
}

// LCOV_EXCL_START - reason: needs a controlling terminal, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
fn record_reported(job: &backend::TerminalJob) {
    while let Some(signo) = job.take_reported() {
        crate::record_interrupt(signo);
    }
}

fn settle_reported(job: &backend::TerminalJob) {
    for signo in job.settle() {
        crate::record_interrupt(signo);
    }
    record_reported(job);
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

struct Tree {
    alive: fn(i32) -> bool,
    send: fn(i32, i32),
    kill: fn(i32),
}

const OS_TREE: Tree = Tree {
    alive: backend::tree_alive,
    send: backend::send,
    kill: backend::send_kill,
};

fn withdraw_leftover(tree: &Tree, target: i32) -> bool {
    let alive = (tree.alive)(target);
    if alive {
        (tree.send)(target, WITHDRAW_SIGNAL);
    }
    alive
}

fn settle_owned_tree(tree: &Tree, target: i32, cleanup: CleanupBudget) {
    if !wait_tree_gone(tree, target, after(Instant::now(), cleanup.graceful)) {
        (tree.kill)(target);
        let _: bool = wait_tree_gone(tree, target, after(Instant::now(), cleanup.forced));
    }
}

fn wait_tree_gone(tree: &Tree, target: i32, until: Instant) -> bool {
    loop {
        if !(tree.alive)(target) {
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
        drop(io::copy(&mut pipe, &mut sink));
        drop(sink.flush());
        drop(pipe);
        tx.send(()).unwrap_or_default();
    });
    rx
}

#[cfg(unix)]
#[path = "terminal.rs"]
mod terminal;

#[cfg(unix)]
#[path = "lifecycle_unix.rs"]
mod unix_backend;

#[cfg(windows)]
#[path = "lifecycle_windows.rs"]
mod windows_backend;

#[cfg(test)]
pub(crate) mod test_exports {
    #[cfg(windows)]
    pub(crate) use super::windows_backend::RecordStop;
}

mod backend {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Group {
        Owned,
        Join(i32),
        Own,
    }

    #[cfg(unix)]
    pub use super::terminal::{TerminalJob, allow_background_tty_writes};

    #[cfg(unix)]
    pub use super::unix_backend::{
        OwnedChild, end_owned_descendants, force_stream, send, send_kill, spawn_owned,
        spawn_streamed, stream_target, terminal_foreground, terminate_forced, terminate_graceful,
        termination_signal, tree_alive,
    };

    #[cfg(windows)]
    pub use super::windows_backend::{
        OwnedChild, TerminalJob, allow_background_tty_writes, end_owned_descendants, force_stream,
        send, send_kill, spawn_owned, spawn_streamed, stream_target, terminal_foreground,
        terminate_forced, terminate_graceful, termination_signal, tree_alive,
    };

    #[cfg(windows)]
    pub use super::windows_backend::install_console_stop;
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
