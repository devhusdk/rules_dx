//! Build slots shared by every dx run on the machine.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::args::Command;
use crate::worktree::host::{self, Host, Server, SystemHost};

/// Set once a slot is held; nested dx calls see it and take no second slot.
pub const SLOT_ENV_VAR: &str = "DX_BUILD_SLOT";

/// Set by Bazel in every test action, so dx inside a test takes no slot.
pub const BAZEL_TEST_ENV_VAR: &str = "BAZEL_TEST";

/// Memory one Bazel server plus its build is planned to use.
pub const BYTES_PER_SLOT: u64 = 3 << 30;

/// Sets how many nested Bazel test servers rules_dx tests run at once.
pub const NESTED_BAZEL_SLOTS_ENV: &str = "DX_NESTED_BAZEL_SLOTS";

/// Nested Bazel test servers rules_dx tests run at once by default.
pub const DEFAULT_NESTED_BAZEL_SLOTS: u64 = 3;

/// Memory one nested Bazel test server is planned to use.
pub const BYTES_PER_NESTED_BAZEL: u64 = 2 << 30;

/// Most slots dx ever offers.
pub const MAX_SLOTS: usize = 16;

const POLL: Duration = Duration::from_millis(200);

const STALE: Duration = Duration::from_secs(30);

/// A held build slot; dropping it, or exiting, frees it.
#[derive(Debug)]
pub struct Slot {
    _file: File,
    _reserve: Vec<File>,
    holder: PathBuf,
    pub index: usize,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.holder);
    }
}

/// Reports whether the command starts Bazel work.
pub fn needs_slot(command: Command) -> bool {
    matches!(
        command,
        Command::Build
            | Command::Test
            | Command::Coverage
            | Command::Lint
            | Command::Typecheck
            | Command::Format
            | Command::Check
            | Command::Fix
            | Command::Deploy
            | Command::Generate
            | Command::Codegen
            | Command::Env
            | Command::Security
            | Command::License
            | Command::Docs
            | Command::ListTests
    )
}

/// Reports whether the command takes slots around its Bazel work instead of for the whole process.
pub fn per_phase(command: Command) -> bool {
    matches!(command, Command::Run | Command::Watch | Command::Update)
}

/// Memory kept for nested Bazel test servers; only the rules_dx workspace runs them.
pub fn nested_reserve(workspace: &Path, env_get: &dyn Fn(&str) -> Option<String>) -> u64 {
    if !workspace.join("cli/cli/BUILD.bazel").is_file() {
        return 0;
    }
    let is_rules_dx = std::fs::read_to_string(workspace.join("MODULE.bazel")).is_ok_and(|text| {
        text.lines()
            .any(|line| line.trim_end_matches('\r') == "    name = \"rules_dx\",")
    });
    if !is_rules_dx {
        return 0;
    }
    let servers = env_get(NESTED_BAZEL_SLOTS_ENV)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_NESTED_BAZEL_SLOTS);
    servers.saturating_mul(BYTES_PER_NESTED_BAZEL)
}

/// Slots for a machine with `memory` bytes, `reserved` of them kept aside.
pub fn slot_count(memory: u64, reserved: u64) -> usize {
    usize::try_from(memory.saturating_sub(reserved) / BYTES_PER_SLOT)
        .unwrap_or(MAX_SLOTS)
        .clamp(1, MAX_SLOTS)
}

/// The slot directory under the user cache.
pub fn slots_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let cache = match non_empty("XDG_CACHE_HOME") {
        Some(cache) => PathBuf::from(cache),
        None => match non_empty("HOME") {
            Some(home) => PathBuf::from(home).join(".cache"),
            None => PathBuf::from(non_empty("LOCALAPPDATA")?),
        },
    };
    Some(cache.join("dx").join("slots"))
}

fn slot_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.lock"))
}

/// Locks a slot file shared or exclusive, falling back to a read-only open when it cannot be written.
fn lock_slot(path: &Path, shared: bool) -> io::Result<Option<File>> {
    let locked = if shared {
        dx_atomic_fs::lease::try_lock_file_shared(path, true)
    } else {
        dx_atomic_fs::lease::try_lock_file(path, true)
    };
    let error = match locked {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => error,
        other => return other,
    };
    let Ok(file) = OpenOptions::new().read(true).open(path) else {
        return Err(error);
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err(error);
    }
    let locked = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    match locked {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(_)) => Err(error),
    }
}

fn holder_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.holder"))
}

fn holder(dir: &Path, count: usize, ahead: usize) -> String {
    let names: Vec<String> = (0..count)
        .filter_map(|index| std::fs::read_to_string(holder_path(dir, index)).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect();
    let first = names.first().map_or("another build", String::as_str);
    let others = match names.len() {
        0 | 1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    let line = match ahead {
        0 => String::new(),
        n => format!(", {n} ahead in line"),
    };
    format!("{first}{others}{line}")
}

fn queue_dir(dir: &Path) -> PathBuf {
    dir.join("queue")
}

fn tickets(dir: &Path) -> Vec<(u64, PathBuf)> {
    std::fs::read_dir(queue_dir(dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let number = name.strip_prefix("ticket-")?.strip_suffix(".lock")?;
            Some((number.parse().ok()?, path))
        })
        .collect()
}

/// A waiter's place in line; dropping it leaves the line.
struct Ticket {
    number: u64,
    file: Option<File>,
    path: PathBuf,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.path);
        drop(self.file.take());
        #[cfg(not(unix))]
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Ticket {
    /// Marks the ticket live and counts the live tickets ahead of it.
    fn ahead(&self, dir: &Path) -> usize {
        if let Some(file) = &self.file {
            let _ = file.set_modified(SystemTime::now());
        }
        tickets(dir)
            .into_iter()
            .filter(|(number, path)| *number < self.number && waiting(path))
            .count()
    }
}

fn waiting(path: &Path) -> bool {
    let fresh = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|time| time.elapsed().unwrap_or_default() < STALE);
    fresh && matches!(dx_atomic_fs::lease::try_lock_file(path, false), Ok(None))
}

/// Takes the next ticket, sweeping tickets whose waiter is gone.
fn enqueue(dir: &Path) -> io::Result<Ticket> {
    let _lock = dx_atomic_fs::lease::acquire_exclusive_at(
        &queue_dir(dir).join("lock"),
        Duration::from_secs(5),
    )?;
    let mut next = 0;
    for (number, path) in tickets(dir) {
        match dx_atomic_fs::lease::try_lock_file(&path, false) {
            Ok(Some(_dead)) => {
                let _ = std::fs::remove_file(&path);
            }
            _ => next = next.max(number + 1),
        }
    }
    let path = queue_dir(dir).join(format!("ticket-{next:020}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let locked = dx_atomic_fs::lock_exclusive(&file, Duration::from_secs(1));
    let ticket = Ticket {
        number: next,
        file: Some(file),
        path,
    };
    locked.map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
    Ok(ticket)
}

/// Takes a free slot without waiting; ignores the line of waiters.
pub fn try_take(dir: &Path, count: usize, label: &str) -> io::Result<Option<Slot>> {
    scan(dir, count, label).0
}

/// Takes a free slot like `try_take` and lists the slot files it could not use.
fn scan(
    dir: &Path,
    count: usize,
    label: &str,
) -> (io::Result<Option<Slot>>, Vec<(PathBuf, io::Error)>) {
    let mut skipped: Vec<(PathBuf, io::Error)> = Vec::new();
    let mut opened = false;
    for index in 0..count {
        let path = slot_path(dir, index);
        match lock_slot(&path, false) {
            Ok(Some(file)) => {
                let holder = holder_path(dir, index);
                let _ = std::fs::write(&holder, label);
                let slot = Slot {
                    _file: file,
                    _reserve: Vec::new(),
                    holder,
                    index,
                };
                return (Ok(Some(slot)), skipped);
            }
            Ok(None) => opened = true,
            Err(error) => skipped.push((path, error)),
        }
    }
    if opened {
        return (Ok(None), skipped);
    }
    match skipped.pop() {
        Some((_, error)) => (Err(error), skipped),
        None => (Ok(None), skipped),
    }
}

/// Takes a slot in arrival order, calling `on_wait` once when it must wait; errors only when no slot file opens.
pub fn acquire(
    dir: &Path,
    count: usize,
    label: &str,
    on_wait: &mut dyn FnMut(&str),
) -> io::Result<Slot> {
    acquire_with(dir, count, label, on_wait, &mut || {}, &mut |_, _| {})
}

/// Takes a slot like `acquire`, calling `on_poll` before each sleep and `on_skip` for each unusable slot file.
fn acquire_with(
    dir: &Path,
    count: usize,
    label: &str,
    on_wait: &mut dyn FnMut(&str),
    on_poll: &mut dyn FnMut(),
    on_skip: &mut dyn FnMut(&Path, &io::Error),
) -> io::Result<Slot> {
    acquire_reserving(dir, count, count..count, label, on_wait, on_poll, on_skip)
}

fn acquire_reserving(
    dir: &Path,
    count: usize,
    reserve: Range<usize>,
    label: &str,
    on_wait: &mut dyn FnMut(&str),
    on_poll: &mut dyn FnMut(),
    on_skip: &mut dyn FnMut(&Path, &io::Error),
) -> io::Result<Slot> {
    let ticket = enqueue(dir).ok();
    let mut pending: Vec<usize> = reserve.clone().collect();
    let mut held: Vec<File> = Vec::new();
    let mut announced = false;
    loop {
        let ahead = ticket.as_ref().map_or(0, |ticket| ticket.ahead(dir));
        if ahead == 0 {
            pending.retain(|&index| {
                let path = slot_path(dir, index);
                match lock_slot(&path, true) {
                    Ok(Some(file)) => {
                        held.push(file);
                        false
                    }
                    Ok(None) => true,
                    Err(error) => {
                        on_skip(&path, &error);
                        false
                    }
                }
            });
            if pending.is_empty() {
                let (taken, skipped) = scan(dir, count, label);
                let taken = taken?;
                for (path, error) in &skipped {
                    on_skip(path, error);
                }
                if let Some(mut slot) = taken {
                    slot._reserve = held;
                    return Ok(slot);
                }
            }
        }
        if !announced {
            announced = true;
            on_wait(&holder(dir, reserve.end.max(count), ahead));
        }
        if let Some(signo) = dx_process::interrupted() {
            return Err(stopped(signo));
        }
        on_poll();
        std::thread::sleep(POLL);
    }
}

fn stopped(signo: i32) -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        format!("stopped by signal {signo}"),
    )
}

/// The one line printed while waiting.
pub fn wait_message(holder: &str) -> String {
    format!("waiting for a build slot (held by {holder})")
}

/// Total memory visible to this process, honoring a cgroup limit.
pub fn available_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let total = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| parse_meminfo(&text));
        let limit = cgroup_limit();
        return match (total, limit) {
            (Some(total), Some(limit)) => Some(total.min(limit)),
            (total, limit) => total.or(limit),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let mut value: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        let name = c"hw.memsize";
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&mut value as *mut u64).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        return (status == 0).then_some(value);
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatus {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        }
        let mut status = MemoryStatus {
            length: std::mem::size_of::<MemoryStatus>() as u32,
            load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page: 0,
            avail_page: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended: 0,
        };
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        return (ok != 0).then_some(status.total_phys);
    }
    #[allow(unreachable_code)]
    None
}

/// Reads `MemTotal` from `/proc/meminfo` text, in bytes.
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

/// Reads a cgroup `memory.max` value; `max` means no limit.
pub fn parse_cgroup_limit(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn cgroup_limit() -> Option<u64> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative = own
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or("/");
    let root = Path::new("/sys/fs/cgroup");
    let mut dir = root.join(relative.trim_start_matches('/'));
    let mut limit: Option<u64> = None;
    loop {
        let found = std::fs::read_to_string(dir.join("memory.max"))
            .ok()
            .and_then(|text| parse_cgroup_limit(&text));
        limit = match (limit, found) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if dir == root || !dir.pop() || !dir.starts_with(root) {
            return limit;
        }
    }
}

/// Where this dx process takes its slots, fixed at startup.
#[derive(Clone, Debug)]
pub struct Gate {
    dir: PathBuf,
    count: usize,
    pool: usize,
    label: String,
    json: bool,
    warm: Option<Warmth>,
    skipped: Arc<Mutex<BTreeSet<PathBuf>>>,
}

impl Gate {
    /// A gate over `count` slots in `dir`.
    pub fn new(dir: PathBuf, count: usize, label: String, json: bool) -> Self {
        Gate {
            dir,
            count,
            pool: count,
            label,
            json,
            warm: None,
            skipped: Arc::default(),
        }
    }

    /// Spans `pool` slots, keeping those above `count` for nested Bazel servers.
    pub fn reserving(self, pool: usize) -> Self {
        Gate {
            pool: pool.max(self.count),
            ..self
        }
    }

    /// The gate for this process; `None` when nested, inside a Bazel test, or without a cache dir.
    pub fn from_env(
        workspace: &Path,
        env_get: &dyn Fn(&str) -> Option<String>,
        json: bool,
    ) -> Option<Self> {
        let set = |name: &str| env_get(name).is_some_and(|value| !value.is_empty());
        if set(SLOT_ENV_VAR) || set(BAZEL_TEST_ENV_VAR) {
            return None;
        }
        let dir = slots_dir(env_get)?;
        let memory = available_memory()?;
        let count = slot_count(memory, nested_reserve(workspace, env_get));
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        let warm = own_startup_options(&args).map(|startup_options| Warmth {
            workspace: workspace.to_path_buf(),
            startup_options,
        });
        Some(Gate {
            warm,
            ..Gate::new(dir, count, workspace.display().to_string(), json)
                .reserving(slot_count(memory, 0))
        })
    }

    /// Holds a slot for the whole process when the command starts Bazel work.
    pub fn hold(
        &self,
        command: Command,
        err: &mut dyn Write,
        events: &mut dyn Write,
    ) -> Result<Option<Slot>, i32> {
        if !needs_slot(command) {
            return Ok(None);
        }
        self.take(command, err, events)
    }

    /// Takes a slot, waiting with one notice when none is free; `Err` carries the stop signal.
    pub fn take(
        &self,
        command: Command,
        err: &mut dyn Write,
        events: &mut dyn Write,
    ) -> Result<Option<Slot>, i32> {
        let json = self.json;
        let mut keep = self.warm.clone().map(KeepWarm::new);
        let out = RefCell::new((err, events));
        let reserve = self.count..self.pool;
        let result = acquire_reserving(
            &self.dir,
            self.count,
            reserve,
            &self.label,
            &mut |holder: &str| {
                let (err, events) = &mut *out.borrow_mut();
                let message = wait_message(holder);
                let _ = writeln!(err, "dx: {message}");
                if json {
                    notice(
                        &mut **events,
                        command,
                        "info",
                        "waiting_for_build_slot",
                        message,
                    );
                }
            },
            &mut || {
                if let Some(keep) = keep.as_mut() {
                    keep.tick(Instant::now());
                }
            },
            &mut |path: &Path, error: &io::Error| {
                let first = self
                    .skipped
                    .lock()
                    .is_ok_and(|mut seen| seen.insert(path.to_path_buf()));
                if !first {
                    return;
                }
                let (err, events) = &mut *out.borrow_mut();
                let message = format!("skipped build slot {}: {error}", path.display());
                let _ = writeln!(err, "dx: {message}");
                if json {
                    notice(
                        &mut **events,
                        command,
                        "warning",
                        "build_slot_skipped",
                        message,
                    );
                }
            },
        );
        drop(keep);
        let (err, events) = out.into_inner();
        match result {
            Ok(slot) => Ok(Some(slot)),
            Err(error) => {
                if let Some(signo) = dx_process::interrupted() {
                    return Err(signo);
                }
                let message = format!("build slots disabled: {error}");
                let _ = writeln!(err, "dx: {message}");
                if json {
                    notice(events, command, "warning", "build_slots_disabled", message);
                }
                Ok(None)
            }
        }
    }
}

/// The worktree and startup options a waiting run pings its Bazel server with.
#[derive(Clone, Debug)]
struct Warmth {
    workspace: PathBuf,
    startup_options: Vec<String>,
}

/// The Bazel startup options on a dx command line, or `None` when it does not parse.
fn own_startup_options(args: &[OsString]) -> Option<Vec<String>> {
    let (_, tokens) = crate::args::tokenizer::selected_identity(args)?;
    tokens
        .iter()
        .map(|token| dx_process::validate_startup_option(token).ok())
        .collect()
}

/// How often a waiting run pings a server with this idle limit.
fn ping_interval(max_idle_secs: Option<u64>) -> Duration {
    let idle = max_idle_secs.unwrap_or(10_800);
    Duration::from_secs(idle / 3).clamp(Duration::from_secs(1), Duration::from_secs(60))
}

/// Reads `--max_idle_secs` from a server's NUL-separated `cmdline` file.
fn idle_limit(cmdline: &str) -> Option<u64> {
    cmdline
        .split(['\0', '\n'])
        .find_map(|arg| arg.strip_prefix("--max_idle_secs="))
        .and_then(|value| value.trim().parse().ok())
}

fn startup_value<'a>(options: &'a [String], name: &str) -> Option<&'a str> {
    options
        .iter()
        .rev()
        .find_map(|option| option.strip_prefix(name)?.strip_prefix('='))
}

fn default_output_base(dir: &Path, root: &Path) -> bool {
    dir.parent()
        .is_some_and(|parent| host::same_path(parent, root))
        && dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.len() == 32 && name.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The one running server a Bazel call from `workspace` with `options` would reach.
fn own_server<'a>(
    servers: &'a [Server],
    workspace: &Path,
    options: &[String],
    default_roots: &[PathBuf],
) -> Option<&'a Server> {
    let base = startup_value(options, "--output_base");
    let root = startup_value(options, "--output_user_root");
    let mut found = servers.iter().filter(|server| {
        let (Some(dir), Some(output_base), Some(user_root)) = (
            server.workspace.as_deref(),
            server.output_base.as_deref(),
            server.output_user_root.as_deref(),
        ) else {
            return false;
        };
        let root_matches = match root {
            Some(root) => host::same_path(user_root, Path::new(root)),
            None => default_roots
                .iter()
                .any(|default| host::same_path(user_root, default)),
        };
        let base_matches = match base {
            Some(base) => host::same_path(output_base, Path::new(base)),
            None => default_output_base(output_base, user_root),
        };
        host::same_path(dir, workspace) && root_matches && base_matches
    });
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// What keeping a server warm needs from the machine.
trait Warm {
    type Ping;
    fn own_server(&self) -> Option<(u32, Option<u64>)>;
    fn alive(&self, pid: u32) -> bool;
    fn ping(&self) -> Option<Self::Ping>;
    fn finished(&self, ping: &mut Self::Ping) -> bool;
    fn stop(&self, ping: Self::Ping);
}

impl Warm for Warmth {
    type Ping = Child;

    fn own_server(&self) -> Option<(u32, Option<u64>)> {
        let servers = SystemHost.servers();
        let server = own_server(
            &servers,
            &self.workspace,
            &self.startup_options,
            &host::default_output_roots(),
        )?;
        let idle = server
            .output_base
            .as_ref()
            .and_then(|base| std::fs::read(base.join("server").join("cmdline")).ok())
            .and_then(|bytes| idle_limit(&String::from_utf8_lossy(&bytes)));
        Some((server.pid, idle))
    }

    fn alive(&self, pid: u32) -> bool {
        SystemHost.process_alive(pid)
    }

    fn ping(&self) -> Option<Child> {
        let argv = dx_process::startup_argv(&self.startup_options);
        let (program, args) = argv.split_first()?;
        std::process::Command::new(program)
            .args(args)
            .args(["--noblock_for_lock", "info", "server_pid"])
            .current_dir(&self.workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    }

    fn finished(&self, ping: &mut Child) -> bool {
        !matches!(ping.try_wait(), Ok(None))
    }

    fn stop(&self, mut ping: Child) {
        let _ = ping.kill();
        let _ = ping.wait();
    }
}

enum Warming<P> {
    Unknown,
    Off,
    On {
        pid: u32,
        every: Duration,
        due: Instant,
        ping: Option<P>,
    },
}

/// Pings a running server on a timer so it does not shut down for idleness.
struct KeepWarm<W: Warm> {
    warm: W,
    state: Warming<W::Ping>,
}

impl<W: Warm> KeepWarm<W> {
    fn new(warm: W) -> Self {
        KeepWarm {
            warm,
            state: Warming::Unknown,
        }
    }

    /// Finds the server on the first call, then pings it whenever a ping is due.
    fn tick(&mut self, now: Instant) {
        if matches!(self.state, Warming::Unknown) {
            self.state = match self.warm.own_server() {
                Some((pid, idle)) => Warming::On {
                    pid,
                    every: ping_interval(idle),
                    due: now,
                    ping: None,
                },
                None => Warming::Off,
            };
        }
        let Warming::On {
            pid,
            every,
            due,
            ping,
        } = &mut self.state
        else {
            return;
        };
        if let Some(running) = ping.as_mut() {
            if !self.warm.finished(running) {
                return;
            }
            *ping = None;
        }
        if now < *due {
            return;
        }
        if !self.warm.alive(*pid) {
            self.state = Warming::Off;
            return;
        }
        match self.warm.ping() {
            Some(child) => {
                *ping = Some(child);
                *due = now + *every;
            }
            None => self.state = Warming::Off,
        }
    }
}

impl<W: Warm> Drop for KeepWarm<W> {
    fn drop(&mut self) {
        if let Warming::On { ping, .. } = &mut self.state {
            if let Some(child) = ping.take() {
                self.warm.stop(child);
            }
        }
    }
}

fn notice(events: &mut dyn Write, command: Command, level: &str, code: &str, message: String) {
    if let Ok(event) = dx_output::notice_event(&dx_output::NoticeEvent {
        level: level.to_owned(),
        code: code.to_owned(),
        message,
        related_command: Some(command.name().to_owned()),
        scope: None,
        path: None,
        language: None,
        import: None,
    }) {
        let _ = dx_output::lifecycle::write_event(&mut *events, &event);
        let _ = events.flush();
    }
}

thread_local! {
    static GATE: RefCell<Option<Gate>> = const { RefCell::new(None) };
}

struct Restore(Option<Gate>);

impl Drop for Restore {
    fn drop(&mut self) {
        let previous = self.0.take();
        GATE.with(|cell| cell.replace(previous));
    }
}

/// Runs `f` with `gate` as the source of per-phase slots.
pub fn with_gate<R>(gate: Option<Gate>, f: impl FnOnce() -> R) -> R {
    let _restore = Restore(GATE.with(|cell| cell.replace(gate)));
    f()
}

/// Takes a per-phase slot from the installed gate; none when no gate is installed.
pub fn take(
    command: Command,
    err: &mut dyn Write,
    events: &mut dyn Write,
) -> Result<Option<Slot>, i32> {
    match GATE.with(|cell| cell.borrow().clone()) {
        Some(gate) => gate.take(command, err, events),
        None => Ok(None),
    }
}

/// Writes `pending` right after the first line written through it.
pub struct AfterFirstLine<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> AfterFirstLine<W> {
    /// Wraps `inner`, holding `pending` until the first newline.
    pub fn new(inner: W, pending: Vec<u8>) -> Self {
        AfterFirstLine { inner, pending }
    }
}

impl<W: Write> Write for AfterFirstLine<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            return self.inner.write(buf);
        }
        match buf.iter().position(|byte| *byte == b'\n') {
            None => self.inner.write(buf),
            Some(end) => {
                self.inner.write_all(&buf[..=end])?;
                let pending = std::mem::take(&mut self.pending);
                self.inner.write_all(&pending)?;
                self.inner.write_all(&buf[end + 1..])?;
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command as Process, Stdio};

    fn hold(
        command: Command,
        workspace: &Path,
        env_get: &dyn Fn(&str) -> Option<String>,
        err: &mut dyn Write,
        json: bool,
        events: &mut dyn Write,
    ) -> Result<Option<Slot>, i32> {
        Gate::from_env(workspace, env_get, json)
            .map_or(Ok(None), |gate| gate.hold(command, err, events))
    }

    const HOLD_ENV: &str = "DX_SLOT_TEST_HOLD";
    const WAIT_ENV: &str = "DX_SLOT_TEST_WAIT";

    #[test]
    fn slot_holder_helper() {
        let Ok(dir) = std::env::var(HOLD_ENV) else {
            return;
        };
        let _slot = acquire(Path::new(&dir), 1, "helper-tree", &mut |_| {}).expect("slot");
        std::fs::write(Path::new(&dir).join("held"), b"1").expect("marker");
        std::thread::sleep(Duration::from_secs(120));
    }

    #[cfg(unix)]
    extern "C" fn record_stop(signo: libc::c_int) {
        dx_process::record_interrupt(signo);
    }

    #[cfg(unix)]
    #[test]
    fn slot_waiter_helper() {
        let Ok(dir) = std::env::var(WAIT_ENV) else {
            return;
        };
        unsafe {
            libc::signal(
                libc::SIGTERM,
                record_stop as *const () as libc::sighandler_t,
            );
        }
        let dir = Path::new(&dir);
        let result = acquire(dir, 1, "waiter-tree", &mut |_| {
            std::fs::write(dir.join("waiting"), b"1").expect("marker");
        });
        match (result, dx_process::interrupted()) {
            (Err(_), Some(signo)) => std::process::exit(dx_process::interrupted_code(signo)),
            _ => std::process::exit(1),
        }
    }

    fn wait_for(path: &Path, what: &str) {
        let start = std::time::Instant::now();
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(30), "{what}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn spawn_holder(dir: &Path) -> std::process::Child {
        let child = Process::new(std::env::current_exe().expect("exe"))
            .arg("slots::tests::slot_holder_helper")
            .arg("--exact")
            .env(HOLD_ENV, dir)
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");
        wait_for(&dir.join("held"), "holder never took the slot");
        child
    }

    #[cfg(unix)]
    #[test]
    fn sigterm_ends_the_wait_with_128_plus_signal() {
        let scratch = dx_test_scratch::scratch("dx-slots-stop-");
        let dir = scratch.path().join("slots");
        let mut holder = spawn_holder(&dir);
        let mut waiter = Process::new(std::env::current_exe().expect("exe"))
            .arg("slots::tests::slot_waiter_helper")
            .arg("--exact")
            .env(WAIT_ENV, &dir)
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn waiter");
        wait_for(&dir.join("waiting"), "waiter never started waiting");
        let pid = i32::try_from(waiter.id()).expect("pid");
        assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
        let sent = std::time::Instant::now();
        let status = loop {
            if let Some(status) = waiter.try_wait().expect("try_wait") {
                break status;
            }
            assert!(
                sent.elapsed() < Duration::from_secs(10),
                "waiter ignored SIGTERM"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(128 + libc::SIGTERM));
        assert!(tickets(&dir).is_empty());
        assert!(holder.try_wait().expect("holder").is_none());
        holder.kill().expect("kill");
        holder.wait().expect("wait");
    }

    #[test]
    fn count_follows_memory_with_bounds() {
        assert_eq!(slot_count(0, 0), 1);
        assert_eq!(slot_count(4 << 30, 0), 1);
        assert_eq!(slot_count(8 << 30, 0), 2);
        assert_eq!(slot_count(16 << 30, 0), 5);
        assert_eq!(slot_count(16 << 30, 6 << 30), 3);
        assert_eq!(slot_count(24 << 30, 0), 8);
        assert_eq!(slot_count(u64::MAX, 0), MAX_SLOTS);
        assert_eq!(slot_count(12 << 30, 6 << 30), 2);
        assert_eq!(slot_count(20 << 30, 6 << 30), 4);
        assert_eq!(slot_count(24 << 30, 6 << 30), 6);
    }

    fn rules_dx_layout(prefix: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(prefix);
        std::fs::create_dir_all(scratch.path().join("cli/cli")).expect("dirs");
        std::fs::write(scratch.path().join("cli/cli/BUILD.bazel"), "").expect("build");
        std::fs::write(
            scratch.path().join("MODULE.bazel"),
            "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\n",
        )
        .expect("module");
        scratch
    }

    #[test]
    fn consumer_workspaces_reserve_nothing() {
        let none = |_: &str| None;
        assert_eq!(nested_reserve(Path::new("/ws"), &none), 0);
        let scratch = dx_test_scratch::scratch("dx-slots-consumer-");
        std::fs::create_dir_all(scratch.path().join("cli/cli")).expect("dirs");
        std::fs::write(scratch.path().join("cli/cli/BUILD.bazel"), "").expect("build");
        std::fs::write(
            scratch.path().join("MODULE.bazel"),
            "module(\n    name = \"my_app\",\n)\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n",
        )
        .expect("module");
        assert_eq!(nested_reserve(scratch.path(), &none), 0);
    }

    #[test]
    fn rules_dx_gate_spans_the_whole_pool() {
        let scratch = rules_dx_layout("dx-slots-pool-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let memory = available_memory().expect("memory");
        let gate = Gate::from_env(scratch.path(), &get, false).expect("gate");
        assert_eq!(gate.count, slot_count(memory, 6 << 30));
        assert_eq!(gate.pool, slot_count(memory, 0));
        let consumer = Gate::from_env(Path::new("/ws"), &get, false).expect("gate");
        assert_eq!(consumer.pool, consumer.count);
        let narrow = Gate::new(PathBuf::from("/slots"), 3, "r".to_owned(), false).reserving(1);
        assert_eq!(narrow.pool, 3);
    }

    #[test]
    fn rules_dx_reserves_the_default_nested_servers() {
        let scratch = rules_dx_layout("dx-slots-rules-dx-");
        assert_eq!(nested_reserve(scratch.path(), &|_| None), 3 * (2 << 30));
    }

    #[test]
    fn rules_dx_reserve_follows_the_nested_slot_env() {
        let scratch = rules_dx_layout("dx-slots-rules-dx-env-");
        let with = |value: &'static str| {
            move |name: &str| (name == "DX_NESTED_BAZEL_SLOTS").then(|| value.to_owned())
        };
        assert_eq!(nested_reserve(scratch.path(), &with("5")), 10 << 30);
        for fallback in ["0", "x", ""] {
            assert_eq!(nested_reserve(scratch.path(), &with(fallback)), 6 << 30);
        }
    }

    #[test]
    fn nested_defaults_match_the_test_harness() {
        assert_eq!(
            usize::try_from(DEFAULT_NESTED_BAZEL_SLOTS).expect("usize"),
            dx_testing::nested::DEFAULT_SLOTS
        );
        assert_eq!(NESTED_BAZEL_SLOTS_ENV, dx_testing::nested::SLOTS_ENV);
    }

    #[test]
    fn memory_text_parses() {
        assert_eq!(
            parse_meminfo("MemFree: 1 kB\nMemTotal:       2048 kB\n"),
            Some(2048 * 1024)
        );
        assert_eq!(parse_meminfo("MemFree: 1 kB\n"), None);
        assert_eq!(parse_cgroup_limit("8589934592\n"), Some(8 << 30));
        assert_eq!(parse_cgroup_limit("max\n"), None);
    }

    #[test]
    fn only_bazel_commands_take_slots() {
        for command in [Command::Build, Command::Test, Command::Coverage] {
            assert!(needs_slot(command));
            assert!(!per_phase(command));
        }
        for command in [Command::Run, Command::Watch, Command::Update] {
            assert!(!needs_slot(command));
            assert!(per_phase(command));
        }
        for command in [
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Deploy,
            Command::Generate,
            Command::Codegen,
            Command::Env,
            Command::Security,
            Command::License,
            Command::Docs,
            Command::ListTests,
        ] {
            assert!(needs_slot(command));
        }
        for command in [
            Command::Status,
            Command::Version,
            Command::Worktree,
            Command::Completion,
        ] {
            assert!(!needs_slot(command));
        }
    }

    #[test]
    fn a_gate_ignores_slot_env_set_after_it_was_built() {
        let scratch = dx_test_scratch::scratch("dx-slots-gate-env-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let nested = std::cell::Cell::new(false);
        let get = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            SLOT_ENV_VAR => nested.get().then(|| "1".to_owned()),
            _ => None,
        };
        let gate = Gate::from_env(Path::new("/ws"), &get, false).expect("gate");
        nested.set(true);
        assert!(Gate::from_env(Path::new("/ws"), &get, false).is_none());
        let slot = with_gate(Some(gate), || {
            take(Command::Run, &mut Vec::new(), &mut Vec::new())
        });
        assert!(matches!(slot, Ok(Some(_))));
        assert!(scratch
            .path()
            .join("dx")
            .join("slots")
            .join("slot-0.lock")
            .exists());
    }

    #[test]
    fn take_without_a_gate_takes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-slots-no-gate-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
        with_gate(Some(gate), || {
            let slot = take(Command::Run, &mut Vec::new(), &mut Vec::new()).expect("take");
            assert!(slot.is_some());
            assert!(with_gate(None, || take(
                Command::Run,
                &mut Vec::new(),
                &mut Vec::new()
            ))
            .expect("none")
            .is_none());
        });
        assert!(take(Command::Run, &mut Vec::new(), &mut Vec::new())
            .expect("none")
            .is_none());
        assert!(try_take(&dir, 1, "after").expect("try").is_some());
    }

    #[test]
    fn cache_dir_follows_the_environment() {
        let get = |name: &str| match name {
            "XDG_CACHE_HOME" => Some("/x".to_owned()),
            _ => None,
        };
        assert_eq!(slots_dir(&get), Some(PathBuf::from("/x/dx/slots")));
        let home = |name: &str| (name == "HOME").then(|| "/h".to_owned());
        assert_eq!(slots_dir(&home), Some(PathBuf::from("/h/.cache/dx/slots")));
        assert_eq!(slots_dir(&|_| None), None);
    }

    #[test]
    fn a_second_taker_waits_until_the_first_drops() {
        let scratch = dx_test_scratch::scratch("dx-slots-wait-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 1, "tree-a", &mut |_| {}).expect("first");
        assert_eq!(first.index, 0);
        let mut seen = Vec::new();
        let second_dir = dir.clone();
        let waiter = std::thread::spawn(move || {
            let mut holders = Vec::new();
            let slot = acquire(&second_dir, 1, "tree-b", &mut |holder| {
                holders.push(holder.to_owned());
            })
            .expect("second");
            (slot.index, holders)
        });
        std::thread::sleep(Duration::from_millis(600));
        drop(first);
        let (index, holders) = waiter.join().expect("join");
        seen.extend(holders);
        assert_eq!(index, 0);
        assert_eq!(seen, vec!["tree-a".to_owned()]);
        assert_eq!(
            wait_message("tree-a"),
            "waiting for a build slot (held by tree-a)"
        );
    }

    #[test]
    fn waiters_get_slots_in_arrival_order() {
        let scratch = dx_test_scratch::scratch("dx-slots-order-");
        let dir = scratch.path().join("slots");
        let held = try_take(&dir, 1, "first").expect("try").expect("free");
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut notices = Vec::new();
        let mut waiters = Vec::new();
        for id in 0..3 {
            let (sender, receiver) = std::sync::mpsc::channel();
            let dir = dir.clone();
            let order = order.clone();
            waiters.push(std::thread::spawn(move || {
                let slot = acquire(&dir, 1, &format!("w{id}"), &mut |holder| {
                    sender.send(holder.to_owned()).expect("send");
                })
                .expect("slot");
                order.lock().expect("order").push(id);
                std::thread::sleep(Duration::from_millis(50));
                drop(slot);
            }));
            notices.push(
                receiver
                    .recv_timeout(Duration::from_secs(30))
                    .expect("notice"),
            );
        }
        drop(held);
        for waiter in waiters {
            waiter.join().expect("join");
        }
        assert_eq!(*order.lock().expect("order"), vec![0, 1, 2]);
        assert_eq!(
            notices,
            vec![
                "first".to_owned(),
                "first, 1 ahead in line".to_owned(),
                "first, 2 ahead in line".to_owned(),
            ]
        );
        assert!(tickets(&dir).is_empty());
    }

    #[test]
    fn a_newcomer_queues_behind_a_waiter() {
        let scratch = dx_test_scratch::scratch("dx-slots-newcomer-");
        let dir = scratch.path().join("slots");
        let queued = enqueue(&dir).expect("queued");
        let (sender, receiver) = std::sync::mpsc::channel();
        let newcomer_dir = dir.clone();
        let newcomer = std::thread::spawn(move || {
            acquire(&newcomer_dir, 1, "newcomer", &mut |holder| {
                sender.send(holder.to_owned()).expect("send");
            })
            .expect("slot")
            .index
        });
        let notice = receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("notice");
        assert_eq!(notice, "another build, 1 ahead in line");
        let first = try_take(&dir, 1, "queued").expect("try").expect("free");
        drop(queued);
        drop(first);
        assert_eq!(newcomer.join().expect("join"), 0);
    }

    #[test]
    fn an_unlocked_ticket_does_not_block_and_is_swept() {
        let scratch = dx_test_scratch::scratch("dx-slots-swept-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(queue_dir(&dir)).expect("queue");
        let dead = queue_dir(&dir).join(format!("ticket-{:020}.lock", 7));
        std::fs::write(&dead, b"").expect("dead ticket");
        std::fs::write(queue_dir(&dir).join("ticket-x.lock"), b"").expect("odd name");
        let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
        assert_eq!(slot.index, 0);
        assert!(!dead.exists());
        assert!(tickets(&dir).is_empty());
    }

    #[test]
    fn a_stale_ticket_does_not_block() {
        let scratch = dx_test_scratch::scratch("dx-slots-stale-");
        let dir = scratch.path().join("slots");
        let stale = enqueue(&dir).expect("stale");
        let earlier = SystemTime::now() - Duration::from_secs(60);
        stale
            .file
            .as_ref()
            .expect("file")
            .set_modified(earlier)
            .expect("mtime");
        let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
        assert_eq!(slot.index, 0);
        assert!(stale.path.exists());
    }

    #[test]
    fn a_broken_queue_still_gates() {
        let scratch = dx_test_scratch::scratch("dx-slots-no-queue-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("slots");
        std::fs::write(queue_dir(&dir), b"").expect("blocker");
        assert!(enqueue(&dir).is_err());
        let held = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("held");
        let (sender, receiver) = std::sync::mpsc::channel();
        let waiter_dir = dir.clone();
        let waiter = std::thread::spawn(move || {
            acquire(&waiter_dir, 1, "b", &mut |holder| {
                sender.send(holder.to_owned()).expect("send");
            })
            .expect("slot")
            .index
        });
        let notice = receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("notice");
        assert_eq!(notice, "a");
        drop(held);
        assert_eq!(waiter.join().expect("join"), 0);
    }

    #[test]
    fn a_freed_slot_clears_its_holder() {
        let scratch = dx_test_scratch::scratch("dx-slots-holder-");
        let dir = scratch.path().join("slots");
        let a = try_take(&dir, 3, "a").expect("try").expect("a");
        let b = try_take(&dir, 3, "b").expect("try").expect("b");
        let c = try_take(&dir, 3, "c").expect("try").expect("c");
        assert_eq!(holder(&dir, 3, 2), "a and 2 more, 2 ahead in line");
        drop(a);
        assert!(!holder_path(&dir, 0).exists());
        assert_eq!(holder(&dir, 3, 0), "b and 1 more");
        drop((b, c));
        assert_eq!(holder(&dir, 3, 0), "another build");
    }

    #[test]
    fn a_free_slot_is_taken_without_waiting() {
        let scratch = dx_test_scratch::scratch("dx-slots-free-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 2, "a", &mut |_| {}).expect("first");
        let second = acquire(&dir, 2, "b", &mut |_| panic!("must not wait")).expect("second");
        assert_eq!((first.index, second.index), (0, 1));
    }

    fn take_gate(gate: &Gate) -> (Slot, String, String) {
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = gate
            .take(Command::Test, &mut err, &mut events)
            .expect("take")
            .expect("slot");
        (
            slot,
            String::from_utf8(err).expect("utf8"),
            String::from_utf8(events).expect("utf8"),
        )
    }

    #[test]
    fn a_rules_dx_run_keeps_consumers_off_the_reserve() {
        let scratch = dx_test_scratch::scratch("dx-slots-reserve-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 1, "r".to_owned(), false).reserving(2);
        let (slot, err, _) = take_gate(&gate);
        assert_eq!(slot.index, 0);
        assert_eq!(err, "");
        assert!(try_take(&dir, 2, "c").expect("try").is_none());
        drop(slot);
        assert!(try_take(&dir, 2, "c").expect("try").is_some());
    }

    #[test]
    fn rules_dx_runs_share_the_reserve() {
        let scratch = dx_test_scratch::scratch("dx-slots-share-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 2, "r".to_owned(), false).reserving(3);
        let (first, first_err, _) = take_gate(&gate);
        let (second, second_err, _) = take_gate(&gate);
        assert_eq!((first.index, second.index), (0, 1));
        assert_eq!((first_err.as_str(), second_err.as_str()), ("", ""));
        assert!(try_take(&dir, 3, "c").expect("try").is_none());
    }

    #[test]
    fn a_rules_dx_run_waits_for_a_consumer_on_the_reserve() {
        let scratch = dx_test_scratch::scratch("dx-slots-reserve-wait-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
        let consumer = try_take(&dir, 2, "consumer")
            .expect("try")
            .expect("consumer");
        assert_eq!(consumer.index, 1);
        drop(fill);
        let gate = Gate::new(dir.clone(), 1, "r".to_owned(), true).reserving(2);
        let waiter = std::thread::spawn(move || {
            let (slot, err, events) = take_gate(&gate);
            (slot.index, err, events)
        });
        std::thread::sleep(Duration::from_millis(600));
        drop(consumer);
        let (index, err, events) = waiter.join().expect("join");
        assert_eq!(index, 0);
        assert_eq!(err, "dx: waiting for a build slot (held by consumer)\n");
        assert_eq!(
            events.matches("waiting_for_build_slot").count(),
            1,
            "{events}"
        );
    }

    #[test]
    fn a_queued_consumer_does_not_take_the_reserve_behind_a_rules_dx_run() {
        let scratch = dx_test_scratch::scratch("dx-slots-reserve-line-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
        let consumer = try_take(&dir, 2, "consumer")
            .expect("try")
            .expect("consumer");
        drop(fill);
        let (sender, receiver) = std::sync::mpsc::channel();
        let rules_dx_dir = dir.clone();
        let rules_dx = std::thread::spawn(move || {
            acquire_reserving(
                &rules_dx_dir,
                1,
                1..2,
                "r",
                &mut |holder| {
                    sender.send(holder.to_owned()).expect("send");
                },
                &mut || {},
                &mut |_, _| {},
            )
            .expect("slot")
        });
        let notice = receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("notice");
        assert_eq!(notice, "consumer");
        let (sender, receiver) = std::sync::mpsc::channel();
        let late_dir = dir.clone();
        let late = std::thread::spawn(move || {
            acquire(&late_dir, 2, "late", &mut |holder| {
                sender.send(holder.to_owned()).expect("send");
            })
            .expect("slot")
            .index
        });
        let notice = receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("notice");
        assert_eq!(notice, "consumer, 1 ahead in line");
        drop(consumer);
        let held = rules_dx.join().expect("join");
        assert_eq!(held.index, 0);
        std::thread::sleep(Duration::from_millis(600));
        assert!(
            !late.is_finished(),
            "a consumer must not take a reserve slot"
        );
        drop(held);
        assert!(late.join().expect("join") < 2);
    }

    #[test]
    fn an_unopenable_slot_file_is_skipped() {
        let scratch = dx_test_scratch::scratch("dx-slots-skip-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
        let slot = acquire(&dir, 2, "a", &mut |_| panic!("must not wait")).expect("slot");
        assert_eq!(slot.index, 1);
    }

    #[cfg(unix)]
    #[test]
    fn read_only_slot_file_still_counts() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-slots-readonly-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = slot_path(&dir, 0);
        std::fs::write(&path, b"").expect("slot file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
        let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
        assert_eq!(slot.index, 0);
        assert!(try_take(&dir, 1, "b").expect("try").is_none());
        drop(slot);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_reserve_slot_still_reserves() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-slots-readonly-reserve-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = slot_path(&dir, 1);
        std::fs::write(&path, b"").expect("slot file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
        let gate = Gate::new(dir.clone(), 1, "r".to_owned(), false).reserving(2);
        let (slot, err, _) = take_gate(&gate);
        assert_eq!((slot.index, err.as_str()), (0, ""));
        assert!(try_take(&dir, 2, "c").expect("try").is_none());
        drop(slot);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    }

    #[test]
    fn a_skipped_slot_warns_once() {
        for json in [true, false] {
            let scratch = dx_test_scratch::scratch("dx-slots-warn-");
            let dir = scratch.path().join("slots");
            std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
            let gate = Gate::new(dir, 2, "a".to_owned(), json);
            let mut err = Vec::new();
            let mut events = Vec::new();
            for _ in 0..2 {
                let slot = gate.clone().take(Command::Test, &mut err, &mut events);
                assert!(matches!(slot, Ok(Some(ref slot)) if slot.index == 1));
            }
            let text = String::from_utf8(err).expect("utf8");
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(lines.len(), 1, "{text}");
            assert!(lines[0].starts_with("dx: skipped build slot "), "{text}");
            assert!(lines[0].contains("slot-0.lock"), "{text}");
            let events = String::from_utf8(events).expect("utf8");
            if json {
                assert_eq!(events.lines().count(), 1, "{events}");
                assert!(events.contains("\"event\":\"notice\""));
                assert!(events.contains("build_slot_skipped"));
                assert!(events.contains("\"level\":\"warning\""));
            } else {
                assert!(events.is_empty(), "{events}");
            }
        }
    }

    #[test]
    fn no_openable_slot_file_is_an_error() {
        let scratch = dx_test_scratch::scratch("dx-slots-none-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
        assert!(try_take(&dir, 1, "a").is_err());
    }

    #[test]
    fn a_bad_slot_and_busy_slots_still_wait() {
        let scratch = dx_test_scratch::scratch("dx-slots-partial-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
        let _held = acquire(&dir, 2, "a", &mut |_| {}).expect("held");
        assert!(try_take(&dir, 2, "b").expect("try").is_none());
    }

    #[test]
    fn a_crashed_holder_frees_its_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-crash-");
        let dir = scratch.path().join("slots");
        let mut child = spawn_holder(&dir);
        assert!(try_take(&dir, 1, "me").expect("try").is_none());
        child.kill().expect("kill");
        child.wait().expect("wait");
        let slot = acquire(&dir, 1, "me", &mut |_| {}).expect("slot after crash");
        assert_eq!(slot.index, 0);
    }

    #[test]
    fn a_nested_call_takes_no_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-nested-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let nested = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            SLOT_ENV_VAR => Some("1".to_owned()),
            _ => None,
        };
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Build,
            Path::new("/ws"),
            &nested,
            &mut err,
            false,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        assert!(!scratch.path().join("dx").exists());
    }

    #[test]
    fn a_dx_call_inside_a_bazel_test_takes_no_slot_when_all_are_full() {
        let scratch = dx_test_scratch::scratch("dx-slots-bazel-test-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"), 0);
        let _held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let inside_test = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            BAZEL_TEST_ENV_VAR => Some("1".to_owned()),
            _ => None,
        };
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &inside_test,
            &mut err,
            true,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        assert!(err.is_empty() && events.is_empty());
    }

    fn busy(count: usize) -> String {
        let more = (1..count)
            .last()
            .map_or(String::new(), |n| format!(" and {n} more"));
        format!("tree-0{more}")
    }

    #[test]
    fn waiting_prints_one_line_and_one_json_notice() {
        let scratch = dx_test_scratch::scratch("dx-slots-notice-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"), 0);
        let held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &get,
            &mut err,
            true,
            &mut events,
        );
        releaser.join().expect("join");
        assert!(matches!(slot, Ok(Some(_))));
        let text = String::from_utf8(err).expect("utf8");
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains(&wait_message(&busy(count))), "{text}");
        let json = String::from_utf8(events).expect("utf8");
        assert_eq!(json.lines().count(), 1);
        assert!(json.contains("\"event\":\"notice\""));
        assert!(json.contains("waiting_for_build_slot"));
    }

    #[test]
    fn unopenable_slots_print_one_disabled_line_and_one_json_notice() {
        let scratch = dx_test_scratch::scratch("dx-slots-disabled-");
        let cache = scratch.path().to_string_lossy().into_owned();
        std::fs::create_dir_all(scratch.path().join("dx")).expect("dx dir");
        std::fs::write(scratch.path().join("dx").join("slots"), b"").expect("blocker");
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        for json in [true, false] {
            let mut err = Vec::new();
            let mut events = Vec::new();
            let slot = hold(
                Command::Test,
                Path::new("/ws"),
                &get,
                &mut err,
                json,
                &mut events,
            );
            assert!(matches!(slot, Ok(None)));
            let text = String::from_utf8(err).expect("utf8");
            assert_eq!(text.lines().count(), 1);
            assert!(text.starts_with("dx: build slots disabled: "), "{text}");
            let lines = String::from_utf8(events).expect("utf8");
            if json {
                assert_eq!(lines.lines().count(), 1);
                assert!(lines.contains("\"event\":\"notice\""));
                assert!(lines.contains("build_slots_disabled"));
                assert!(lines.contains("\"level\":\"warning\""));
            } else {
                assert!(lines.is_empty());
            }
        }
    }

    #[test]
    fn list_tests_waits_for_a_slot() {
        let command = Command::ListTests;
        let scratch = dx_test_scratch::scratch("dx-slots-list-tests-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"), 0);
        let held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            command,
            Path::new("/ws"),
            &get,
            &mut err,
            false,
            &mut events,
        );
        releaser.join().expect("join");
        assert!(matches!(slot, Ok(Some(_))), "{}", command.name());
        let text = String::from_utf8(err).expect("utf8");
        assert!(text.contains(&wait_message(&busy(count))), "{text}");
        let nested = |name: &str| (name == SLOT_ENV_VAR).then(|| "1".to_owned());
        let slot = hold(
            command,
            Path::new("/ws"),
            &nested,
            &mut Vec::new(),
            false,
            &mut Vec::new(),
        );
        assert!(matches!(slot, Ok(None)));
    }

    #[test]
    fn update_holds_no_process_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-update-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"), 0);
        let held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Update,
            Path::new("/ws"),
            &get,
            &mut err,
            true,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
        assert!(events.is_empty(), "{}", String::from_utf8_lossy(&events));
        drop(held);
    }

    #[test]
    fn a_waiter_polls_while_it_waits_and_stops_once_it_takes_the_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-poll-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 1, "tree-a", &mut |_| {}).expect("first");
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = polls.clone();
        let waiter_dir = dir.clone();
        let waiter = std::thread::spawn(move || {
            acquire_with(
                &waiter_dir,
                1,
                "tree-b",
                &mut |_| {},
                &mut || {
                    seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                },
                &mut |_, _| {},
            )
            .expect("second")
        });
        let start = Instant::now();
        while polls.load(std::sync::atomic::Ordering::SeqCst) < 2 {
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "waiter never polled"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        drop(first);
        let slot = waiter.join().expect("join");
        let after = polls.load(std::sync::atomic::Ordering::SeqCst);
        assert!(after >= 2);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(polls.load(std::sync::atomic::Ordering::SeqCst), after);
        drop(slot);
    }

    #[test]
    fn a_free_slot_never_polls() {
        let scratch = dx_test_scratch::scratch("dx-slots-nopoll-");
        let dir = scratch.path().join("slots");
        let mut polls = 0;
        let slot = acquire_with(
            &dir,
            1,
            "tree",
            &mut |_| {},
            &mut || polls += 1,
            &mut |_, _| {},
        )
        .expect("slot");
        assert_eq!(slot.index, 0);
        assert_eq!(polls, 0);
    }

    #[test]
    fn ping_interval_is_a_third_of_the_idle_limit_within_bounds() {
        assert_eq!(ping_interval(Some(90)), Duration::from_secs(30));
        assert_eq!(ping_interval(Some(300)), Duration::from_secs(60));
        assert_eq!(ping_interval(Some(900)), Duration::from_secs(60));
        assert_eq!(ping_interval(None), Duration::from_secs(60));
        assert_eq!(ping_interval(Some(2)), Duration::from_secs(1));
        assert_eq!(ping_interval(Some(0)), Duration::from_secs(1));
    }

    #[test]
    fn idle_limit_is_read_from_the_server_cmdline() {
        let cmdline = "bazel(ws)\0-jar\0/i/A-server.jar\0--max_idle_secs=90\0--output_base=/o\0";
        assert_eq!(idle_limit(cmdline), Some(90));
        assert_eq!(idle_limit("bazel(ws)\0--output_base=/o\0"), None);
        assert_eq!(idle_limit("--max_idle_secs=x\0"), None);
    }

    #[test]
    fn startup_options_come_from_the_dx_command_line() {
        let words = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(
            own_startup_options(&words(&["test", "//..."])),
            Some(Vec::new())
        );
        assert_eq!(
            own_startup_options(&words(&[
                "test",
                "--bazel-startup-option=--output_base=/o",
                "//..."
            ])),
            Some(vec!["--output_base=/o".to_owned()])
        );
        assert_eq!(own_startup_options(&words(&["slots::"])), None);
    }

    const BASE: &str = "0123456789abcdef0123456789abcdef";

    fn server(pid: u32, workspace: &str, root: &str, base: &str) -> Server {
        Server {
            pid,
            rss_bytes: 0,
            workspace: Some(PathBuf::from(workspace)),
            output_base: Some(PathBuf::from(base)),
            output_user_root: Some(PathBuf::from(root)),
        }
    }

    #[test]
    fn own_server_is_the_one_this_workspace_reaches() {
        let roots = vec![PathBuf::from("/c/_bazel_u")];
        let base = format!("/c/_bazel_u/{BASE}");
        let servers = vec![
            server(
                1,
                "/other",
                "/c/_bazel_u",
                "/c/_bazel_u/ffffffffffffffffffffffffffffffff",
            ),
            server(2, "/ws", "/c/_bazel_u", &base),
        ];
        let pid = |options: &[&str]| {
            let options: Vec<String> = options.iter().map(|o| (*o).to_owned()).collect();
            own_server(&servers, Path::new("/ws"), &options, &roots).map(|found| found.pid)
        };
        assert_eq!(pid(&[]), Some(2));
        assert_eq!(pid(&[&format!("--output_base={base}")]), Some(2));
        assert_eq!(pid(&["--output_base=/elsewhere"]), None);
        assert_eq!(pid(&["--output_user_root=/elsewhere"]), None);
        assert_eq!(
            own_server(&servers, Path::new("/nope"), &[], &roots).map(|found| found.pid),
            None
        );
    }

    #[test]
    fn own_server_skips_servers_a_plain_call_would_not_reach() {
        let roots = vec![PathBuf::from("/c/_bazel_u")];
        let custom = vec![server(3, "/ws", "/c/_bazel_u", "/c/_bazel_u/custom")];
        assert!(own_server(&custom, Path::new("/ws"), &[], &roots).is_none());
        let other_root = vec![server(4, "/ws", "/r", &format!("/r/{BASE}"))];
        assert!(own_server(&other_root, Path::new("/ws"), &[], &roots).is_none());
        assert_eq!(
            own_server(
                &other_root,
                Path::new("/ws"),
                &["--output_user_root=/r".to_owned()],
                &roots
            )
            .map(|found| found.pid),
            Some(4)
        );
        let two = vec![
            server(5, "/ws", "/c/_bazel_u", &format!("/c/_bazel_u/{BASE}")),
            server(
                6,
                "/ws",
                "/c/_bazel_u",
                "/c/_bazel_u/fedcba9876543210fedcba9876543210",
            ),
        ];
        assert!(own_server(&two, Path::new("/ws"), &[], &roots).is_none());
    }

    #[derive(Default)]
    struct FakeLog {
        lookups: usize,
        pings: usize,
        stopped: usize,
        running: bool,
        dead: bool,
    }

    struct FakeWarm {
        server: Option<(u32, Option<u64>)>,
        log: std::rc::Rc<RefCell<FakeLog>>,
    }

    impl Warm for FakeWarm {
        type Ping = ();

        fn own_server(&self) -> Option<(u32, Option<u64>)> {
            self.log.borrow_mut().lookups += 1;
            self.server
        }

        fn alive(&self, _pid: u32) -> bool {
            !self.log.borrow().dead
        }

        fn ping(&self) -> Option<()> {
            let mut log = self.log.borrow_mut();
            assert!(!log.running, "pings overlap");
            log.pings += 1;
            log.running = true;
            Some(())
        }

        fn finished(&self, _ping: &mut ()) -> bool {
            !self.log.borrow().running
        }

        fn stop(&self, _ping: ()) {
            let mut log = self.log.borrow_mut();
            log.stopped += 1;
            log.running = false;
        }
    }

    fn keeper(
        server: Option<(u32, Option<u64>)>,
    ) -> (KeepWarm<FakeWarm>, std::rc::Rc<RefCell<FakeLog>>) {
        let log = std::rc::Rc::new(RefCell::new(FakeLog::default()));
        let warm = FakeWarm {
            server,
            log: log.clone(),
        };
        (KeepWarm::new(warm), log)
    }

    #[test]
    fn no_running_server_means_no_ping() {
        let (mut keep, log) = keeper(None);
        let start = Instant::now();
        for step in 0..10 {
            keep.tick(start + Duration::from_secs(step * 100));
        }
        drop(keep);
        let log = log.borrow();
        assert_eq!(log.lookups, 1);
        assert_eq!(log.pings, 0);
        assert_eq!(log.stopped, 0);
    }

    #[test]
    fn the_first_tick_pings_and_later_pings_wait_for_the_interval() {
        let (mut keep, log) = keeper(Some((7, Some(90))));
        let start = Instant::now();
        keep.tick(start);
        assert_eq!(log.borrow().pings, 1);
        log.borrow_mut().running = false;
        keep.tick(start + Duration::from_secs(29));
        assert_eq!(log.borrow().pings, 1);
        keep.tick(start + Duration::from_secs(30));
        assert_eq!(log.borrow().pings, 2);
        assert_eq!(log.borrow().lookups, 1);
    }

    #[test]
    fn a_running_ping_is_never_doubled() {
        let (mut keep, log) = keeper(Some((7, Some(3))));
        let start = Instant::now();
        keep.tick(start);
        keep.tick(start + Duration::from_secs(5));
        keep.tick(start + Duration::from_secs(10));
        assert_eq!(log.borrow().pings, 1);
        log.borrow_mut().running = false;
        keep.tick(start + Duration::from_secs(11));
        assert_eq!(log.borrow().pings, 2);
    }

    #[test]
    fn a_dead_server_stops_the_pings() {
        let (mut keep, log) = keeper(Some((7, Some(3))));
        let start = Instant::now();
        keep.tick(start);
        {
            let mut log = log.borrow_mut();
            log.running = false;
            log.dead = true;
        }
        keep.tick(start + Duration::from_secs(1));
        log.borrow_mut().dead = false;
        keep.tick(start + Duration::from_secs(100));
        assert_eq!(log.borrow().pings, 1);
    }

    #[test]
    fn dropping_stops_a_running_ping() {
        let (mut keep, log) = keeper(Some((7, None)));
        keep.tick(Instant::now());
        assert!(log.borrow().running);
        drop(keep);
        assert_eq!(log.borrow().stopped, 1);
        assert!(!log.borrow().running);
        let (mut idle, idle_log) = keeper(Some((8, Some(900))));
        idle.tick(Instant::now());
        idle_log.borrow_mut().running = false;
        idle.tick(Instant::now());
        drop(idle);
        assert_eq!(idle_log.borrow().stopped, 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_real_ping_child_is_reaped_on_stop() {
        let warmth = Warmth {
            workspace: std::env::temp_dir(),
            startup_options: Vec::new(),
        };
        let mut child = Process::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");
        assert!(!warmth.finished(&mut child));
        let pid = i32::try_from(child.id()).expect("pid");
        warmth.stop(child);
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
    }
}
