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
use dx_adopt::defaults::BuildSlots;
pub use dx_process::slot_pool::{
    available_memory, cgroup_limit_in, lock_slot, parse_cgroup_limit, parse_meminfo, slot_count,
    slot_path, slots_dir, BYTES_PER_NESTED_BAZEL, BYTES_PER_SLOT, MAX_SLOTS,
};
use dx_worktree::host::{self, Host, Server, SystemHost};

/// Notice code for the one info notice a run prints while it waits for a free build slot.
pub const WAITING_FOR_BUILD_SLOT_CODE: &str = "waiting_for_build_slot";
/// Notice code for a build slot the run could not take and skipped.
pub const BUILD_SLOT_SKIPPED_CODE: &str = "build_slot_skipped";
/// Notice code for a run that turned build slots off after an error.
pub const BUILD_SLOTS_DISABLED_CODE: &str = "build_slots_disabled";
/// Notice code for the disk reclaim a build slot runs before a build.
pub const DISK_LOW_CODE: &str = "disk_low";

pub mod pressure;

use pressure::{HostPressure, NoSignal, Pressure, PressurePause, PRESSURE_WAIT};

/// Set once a slot is held; nested dx calls see it and take no second slot.
pub const SLOT_ENV_VAR: &str = "DX_BUILD_SLOT";

/// The held slot's CPU and memory share, as `cpu=N,memory=M`; nested dx calls reuse it.
pub const SHARE_ENV_VAR: &str = "DX_BUILD_SHARE";

/// Set by Bazel in every test action, so dx inside a test takes no slot.
pub const BAZEL_TEST_ENV_VAR: &str = "BAZEL_TEST";

/// Memory one remote-execution Bazel server plus its build is planned to use.
pub const BYTES_PER_REMOTE_SLOT: u64 = 3 << 29;

/// Fewest seconds between two extra slot grants.
const EXTRA_SPACING: Duration = Duration::from_secs(10);

/// Sets how many nested Bazel test servers rules_dx tests run at once.
pub const NESTED_BAZEL_SLOTS_ENV: &str = dx_path::nested_bazel::SLOTS_ENV;

/// Nested Bazel test servers rules_dx tests run at once by default.
pub const DEFAULT_NESTED_BAZEL_SLOTS: u64 = dx_path::nested_bazel::DEFAULT_SLOTS as u64;

const POLL: Duration = Duration::from_millis(200);

const STALE: Duration = Duration::from_secs(30);

/// How long a Bazel server must sit unused before a waiter may stop it; above the longest keep-warm ping interval.
const REAP_MIN_IDLE: Duration = Duration::from_secs(120);

/// A held build slot; dropping it, or exiting, frees it.
#[derive(Debug)]
pub struct Slot {
    _file: File,
    _reserve: Vec<File>,
    holder: PathBuf,
    pub index: usize,
    sizing: Option<Arc<ShareSizing>>,
    token: Option<dx_process::ShareToken>,
}

/// What a slot needs to work out its share of the host from the slots held right now.
#[derive(Debug)]
struct ShareSizing {
    dir: PathBuf,
    indices: Vec<usize>,
    own: usize,
    resources: Resources,
}

impl ShareSizing {
    fn share(&self) -> dx_process::LocalShare {
        let held = held(&self.dir, self.indices.iter().copied(), self.own);
        local_share(
            self.resources.cpus,
            self.resources.memory,
            self.resources.reserved,
            held,
        )
    }
}

impl Slot {
    /// The share this slot gets now, split over the slots held at this moment.
    pub fn share(&self) -> Option<dx_process::LocalShare> {
        self.sizing.as_ref().map(|sizing| sizing.share())
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        // Clear the source before the holder goes, so no share runs against a half-removed slot.
        if let Some(token) = self.token.take() {
            dx_process::clear_share_source(token);
        }
        let _ = std::fs::remove_file(&self.holder);
    }
}

/// A runner whose children see `DX_BUILD_SLOT=1` and the slot's share while the slot is held.
pub struct SlotEnv<'a> {
    inner: &'a dyn dx_process::Runner,
    sizing: Option<Arc<ShareSizing>>,
}

impl SlotEnv<'_> {
    /// The share as of this call, so a child started later sees the current split.
    fn share_env(&self) -> Option<String> {
        self.sizing.as_ref().map(|sizing| sizing.share().to_env())
    }
}

impl Slot {
    /// Wraps `inner` so the children it starts know this slot is held.
    pub fn runner<'a>(&'a self, inner: &'a dyn dx_process::Runner) -> SlotEnv<'a> {
        SlotEnv {
            inner,
            sizing: self.sizing.clone(),
        }
    }
}

/// The caller env plus the held-slot flag and share.
fn slot_env<'e>(env: &[(&'e str, &'e str)], share: Option<&'e str>) -> Vec<(&'e str, &'e str)> {
    let mut env = env.to_vec();
    env.push((SLOT_ENV_VAR, "1"));
    if let Some(share) = share {
        env.push((SHARE_ENV_VAR, share));
    }
    env
}

/// Reuses the parent's share in a nested dx call.
pub fn inherit_share(env_get: &dyn Fn(&str) -> Option<String>) {
    if env_get(SLOT_ENV_VAR).is_none_or(|value| value.is_empty()) {
        return;
    }
    if let Some(share) =
        env_get(SHARE_ENV_VAR).and_then(|value| dx_process::LocalShare::parse(&value))
    {
        dx_process::set_share(Some(share));
    }
}

impl dx_process::Runner for SlotEnv<'_> {
    fn run(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        let share = self.share_env();
        self.inner.run(argv, cwd, &slot_env(env, share.as_deref()))
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        self.inner.run_hermetic(argv, cwd, env)
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<dx_process::ChildStatus>> {
        let share = self.share_env();
        self.inner
            .run_withdrawable(argv, cwd, &slot_env(env, share.as_deref()), withdraw)
    }

    fn run_app(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: Option<&dyn Fn() -> bool>,
        held: &[&std::fs::File],
    ) -> io::Result<Option<dx_process::ChildStatus>> {
        let share = self.share_env();
        self.inner.run_app(
            argv,
            cwd,
            &slot_env(env, share.as_deref()),
            withdraw,
            held,
        )
    }

    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        self.inner.pinned_tool(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        self.inner.tool_version(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
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
    let servers = dx_path::nested_bazel::servers(env_get(NESTED_BAZEL_SLOTS_ENV).as_deref());
    u64::from(servers).saturating_mul(BYTES_PER_NESTED_BAZEL)
}

/// Slots for remote-execution runs on a machine with `memory` bytes, `reserved` of them kept aside.
pub fn remote_slot_count(memory: u64, reserved: u64) -> usize {
    usize::try_from(memory.saturating_sub(reserved) / BYTES_PER_REMOTE_SLOT)
        .unwrap_or(MAX_SLOTS)
        .clamp(1, MAX_SLOTS)
}

/// How many slots a gate counts, spans, and adds for remote runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizing {
    /// Slots a run may take.
    pub count: usize,
    /// Slots the gate spans; those above `count` are the nested Bazel reserve.
    pub pool: usize,
    /// Remote-only slots above the pool.
    pub extras: usize,
}

/// The gate sizing for `setting` on a machine with `memory` bytes, `reserved` of them for nested Bazel.
///
/// `Auto` sizes from memory. `Count(n)` takes exactly `n`, keeps the reserve's width above `n`,
/// and adds no remote extras. `Off` has no gate.
pub fn sizing(setting: BuildSlots, memory: u64, reserved: u64) -> Option<Sizing> {
    match setting {
        BuildSlots::Off => None,
        BuildSlots::Auto => {
            let count = slot_count(memory, reserved);
            Some(Sizing {
                count,
                pool: slot_count(memory, 0).max(count),
                extras: remote_slot_count(memory, reserved).saturating_sub(count),
            })
        }
        BuildSlots::Count(count) => {
            let count = count.get();
            let width = slot_count(memory, 0).saturating_sub(slot_count(memory, reserved));
            Some(Sizing {
                count,
                pool: count + width,
                extras: 0,
            })
        }
    }
}

/// One slot's share of `cpus` and of `memory` bytes less `reserved`, split over `held` slots.
pub fn local_share(cpus: u64, memory: u64, reserved: u64, held: usize) -> dx_process::LocalShare {
    let held = u64::try_from(held.max(1)).unwrap_or(u64::MAX);
    dx_process::LocalShare {
        cpus: (cpus / held).max(2),
        memory_mb: (memory.saturating_sub(reserved) / held / (1 << 20)).max(1),
    }
}

/// Build slots held right now and the share a run started now would get, probing the locks read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareReport {
    pub held: usize,
    pub count: usize,
    pub next: dx_process::LocalShare,
}

/// The held slots and the next run's share for `setting`; `None` when the gate is off or memory is unknown.
pub fn share_report(
    workspace: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
    setting: BuildSlots,
) -> Option<ShareReport> {
    let dir = slots_dir(env_get)?;
    let memory = available_memory()?;
    let reserved = nested_reserve(workspace, env_get);
    let size = sizing(setting, memory, reserved)?;
    let cpus = std::thread::available_parallelism().map_or(1, |cpus| cpus.get());
    let cpus = u64::try_from(cpus).unwrap_or(1);
    // `usize::MAX` is no slot, so `held` counts every held slot plus the new run.
    let with_new = held(&dir, 0..size.count, usize::MAX);
    Some(ShareReport {
        held: with_new - 1,
        count: size.count,
        next: local_share(cpus, memory, reserved, with_new),
    })
}

/// Lists the `slot-N.lock` files in `dir` that dx cannot open, without creating or locking anything.
pub fn unusable_slot_files(dir: &Path) -> Vec<(PathBuf, io::Error)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, io::Error)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            name.to_str()
                .and_then(|n| n.strip_prefix("slot-")?.strip_suffix(".lock"))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .filter_map(|entry| {
            let path = entry.path();
            match OpenOptions::new().read(true).open(&path) {
                Err(error) => Some((path, error)),
                Ok(file) if !file.metadata().is_ok_and(|meta| meta.is_file()) => {
                    Some((path, io::Error::other("not a regular file")))
                }
                Ok(_) => None,
            }
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn holder_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.holder"))
}

/// Slots in `indices` held right now, counting `own`.
fn held(dir: &Path, indices: impl Iterator<Item = usize>, own: usize) -> usize {
    let others = indices
        .filter(|&index| index != own && holder_path(dir, index).exists())
        .filter(|&index| matches!(lock_slot(&slot_path(dir, index), true), Ok(None)))
        .count();
    1 + others
}

fn holder(dir: &Path, indices: impl Iterator<Item = usize>, ahead: usize) -> String {
    let names: Vec<String> = indices
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

/// Workspaces whose servers a waiter must not stop: slot holders, live waiters, and its own.
fn kept_workspaces(dir: &Path, indices: impl Iterator<Item = usize>, own: &str) -> Vec<PathBuf> {
    let read = |path: &Path| std::fs::read_to_string(path).ok();
    let holders = indices
        .map(|index| holder_path(dir, index))
        .filter(|path| path.exists())
        .filter_map(|path| read(&path));
    let waiters = tickets(dir)
        .into_iter()
        .filter(|(_, path)| waiting(path))
        .filter_map(|(_, path)| read(&path));
    let mut kept: Vec<PathBuf> = Vec::new();
    for label in std::iter::once(own.to_owned())
        .chain(holders)
        .chain(waiters)
    {
        let label = label.trim();
        if !label.is_empty() && !kept.iter().any(|seen| seen == Path::new(label)) {
            kept.push(PathBuf::from(label));
        }
    }
    kept
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

/// A waiter's place in line; dropping it leaves the line. The ticket file is
/// removed before it is unlocked, so no sweep can reuse its number while the
/// file is still on disk.
struct Ticket {
    number: u64,
    file: Option<File>,
    path: PathBuf,
}

impl Ticket {
    /// Removes the ticket path, runs `between`, then releases the lock. Does
    /// nothing once the file is gone, so a later drop cannot remove a path a
    /// newer waiter has reused.
    fn leave(&mut self, between: impl FnOnce()) {
        if let Some(file) = self.file.take() {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(want_path(&self.path));
            between();
            drop(file);
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.leave(|| {});
    }
}

impl Ticket {
    /// Marks the ticket live, counts the live tickets ahead of it, and lists the
    /// slot indices those tickets could use right now.
    fn ahead(&self, dir: &Path) -> (usize, Vec<Range<usize>>) {
        if let Some(file) = &self.file {
            let _ = file.set_modified(SystemTime::now());
        }
        let mut count = 0;
        let mut forbidden: Vec<Range<usize>> = Vec::new();
        for (number, path) in tickets(dir) {
            if number >= self.number || !waiting(&path) {
                continue;
            }
            count += 1;
            let want = Want::read(&want_path(&path));
            forbidden.push(want.reserve.clone());
            if want.ready(dir) {
                forbidden.push(want.base.clone());
                if want.remote {
                    forbidden.push(want.extras.clone());
                }
            }
        }
        (count, forbidden)
    }
}

/// The slot indices a waiter may take, published next to its ticket so later
/// waiters can pass it when it cannot use a free slot. A missing or unreadable
/// record wants every index, which keeps the line strict.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Want {
    base: Range<usize>,
    reserve: Range<usize>,
    extras: Range<usize>,
    remote: bool,
}

impl Want {
    fn everything() -> Self {
        Self {
            base: 0..usize::MAX,
            reserve: 0..usize::MAX,
            extras: 0..usize::MAX,
            remote: true,
        }
    }

    fn of(layout: &Layout, remote: bool) -> Self {
        Self {
            base: 0..layout.count,
            reserve: layout.reserve.clone(),
            extras: layout.extras.clone(),
            remote,
        }
    }

    fn render(&self) -> String {
        let range = |r: &Range<usize>| format!("{}..{}", r.start, r.end);
        format!(
            "v=1 base={} reserve={} extras={} remote={}\n",
            range(&self.base),
            range(&self.reserve),
            range(&self.extras),
            u8::from(self.remote)
        )
    }

    fn parse(text: &str) -> Option<Self> {
        let range = |value: &str| -> Option<Range<usize>> {
            let (start, end) = value.split_once("..")?;
            Some(start.parse().ok()?..end.parse().ok()?)
        };
        let (mut base, mut reserve, mut extras, mut remote, mut version) =
            (None, None, None, None, false);
        for field in text.split_whitespace() {
            let (key, value) = field.split_once('=')?;
            match key {
                "v" => version = value == "1",
                "base" => base = range(value),
                "reserve" => reserve = range(value),
                "extras" => extras = range(value),
                "remote" => remote = Some(value == "1"),
                _ => {}
            }
        }
        version.then_some(())?;
        Some(Self {
            base: base?,
            reserve: reserve?,
            extras: extras?,
            remote: remote?,
        })
    }

    fn read(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| Self::parse(&text))
            .unwrap_or_else(Self::everything)
    }

    /// Whether every reserve index can take a shared lock now. The probe locks
    /// are dropped at once.
    fn ready(&self, dir: &Path) -> bool {
        self.reserve.end < usize::MAX
            && self
                .reserve
                .clone()
                .all(|index| !matches!(lock_slot(&slot_path(dir, index), true), Ok(None)))
    }
}

fn want_path(ticket: &Path) -> PathBuf {
    ticket.with_extension("want")
}

fn is_forbidden(forbidden: &[Range<usize>], index: usize) -> bool {
    forbidden.iter().any(|range| range.contains(&index))
}

fn waiting(path: &Path) -> bool {
    let fresh = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|time| time.elapsed().unwrap_or_default() < STALE);
    fresh && matches!(dx_atomic_fs::lease::try_lock_file(path, false), Ok(None))
}

/// Takes the next ticket, sweeping tickets whose waiter is gone.
#[cfg(test)]
fn enqueue(dir: &Path, label: &str) -> io::Result<Ticket> {
    enqueue_wanting(dir, label, &Want::everything())
}

/// Takes the next ticket and publishes what the waiter wants beside it.
fn enqueue_wanting(dir: &Path, label: &str, want: &Want) -> io::Result<Ticket> {
    let _lock = dx_atomic_fs::lease::acquire_exclusive_at(
        &queue_dir(dir).join("lock"),
        Duration::from_secs(5),
    )?;
    let mut next = 0;
    for (number, path) in tickets(dir) {
        match dx_atomic_fs::lease::try_lock_file(&path, false) {
            Ok(Some(_dead)) => {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(want_path(&path));
            }
            _ => next = next.max(number + 1),
        }
    }
    sweep_orphan_wants(dir);
    let path = queue_dir(dir).join(format!("ticket-{next:020}.lock"));
    let _ = std::fs::write(want_path(&path), want.render());
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let _ = (&file).write_all(label.as_bytes());
    let locked = dx_atomic_fs::lock_exclusive(&file, Duration::from_secs(1));
    let ticket = Ticket {
        number: next,
        file: Some(file),
        path,
    };
    locked.map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
    Ok(ticket)
}

/// Removes want records whose ticket file is gone. Runs under the queue lock.
fn sweep_orphan_wants(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(queue_dir(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "want")
            && !path.with_extension("lock").exists()
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Takes a free slot without waiting; ignores the line of waiters.
pub fn try_take(dir: &Path, count: usize, label: &str) -> io::Result<Option<Slot>> {
    scan(dir, 0..count, label, &|_| true).0
}

/// Takes a free slot like `try_take` and lists the slot files it could not use.
fn scan(
    dir: &Path,
    indices: Range<usize>,
    label: &str,
    allowed: &dyn Fn(usize) -> bool,
) -> (io::Result<Option<Slot>>, Vec<(PathBuf, io::Error)>) {
    let mut skipped: Vec<(PathBuf, io::Error)> = Vec::new();
    let mut opened = false;
    for index in indices.filter(|&index| allowed(index)) {
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
                    sizing: None,
                    token: None,
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
    acquire_with(
        dir,
        count,
        label,
        &mut |holder, _| on_wait(holder),
        &mut || {},
        &mut |_, _| {},
    )
}

/// Takes a slot like `acquire`, calling `on_poll` before each sleep and `on_skip` for each unusable slot file.
fn acquire_with(
    dir: &Path,
    count: usize,
    label: &str,
    on_wait: &mut dyn FnMut(&str, WaitReason),
    on_poll: &mut dyn FnMut(),
    on_skip: &mut dyn FnMut(&Path, &io::Error),
) -> io::Result<Slot> {
    let layout = Layout {
        count,
        reserve: count..count,
        extras: count..count,
    };
    acquire_reserving(
        dir,
        &layout,
        Admission::none(),
        label,
        on_wait,
        on_poll,
        on_skip,
    )
}

/// Where a gate's slots sit: base `0..count`, the nested reserve, then remote-only extras.
#[derive(Clone, Debug)]
struct Layout {
    count: usize,
    reserve: Range<usize>,
    extras: Range<usize>,
}

impl Layout {
    /// Every slot index a held count or holder list looks at.
    fn counted(&self) -> impl Iterator<Item = usize> {
        (0..self.count).chain(self.extras.clone())
    }

    /// Every slot index a waiter names in its notice.
    fn named(&self) -> impl Iterator<Item = usize> {
        (0..self.reserve.end.max(self.count)).chain(self.extras.clone())
    }
}

fn extra_mark(dir: &Path) -> PathBuf {
    dir.join("extra-granted")
}

/// Whether an extra slot was granted within the last `EXTRA_SPACING`.
fn extra_recent(dir: &Path) -> bool {
    std::fs::metadata(extra_mark(dir))
        .and_then(|meta| meta.modified())
        .is_ok_and(|time| time.elapsed().unwrap_or_default() < EXTRA_SPACING)
}

/// Whether any slot in `extras` is free right now.
fn extra_free(dir: &Path, extras: Range<usize>) -> bool {
    extras
        .into_iter()
        .any(|index| matches!(lock_slot(&slot_path(dir, index), true), Ok(Some(_))))
}

/// Takes a free extra slot when the host is calm and no extra was granted just now.
fn take_extra(
    dir: &Path,
    extras: Range<usize>,
    label: &str,
    high: Option<bool>,
    allowed: &dyn Fn(usize) -> bool,
) -> Option<Slot> {
    if extras.is_empty() || extra_recent(dir) || high != Some(false) {
        return None;
    }
    let (taken, _) = scan(dir, extras, label, allowed);
    let slot = taken.ok().flatten()?;
    let _ = std::fs::write(extra_mark(dir), label);
    Some(slot)
}

/// Why a waiter is not in a slot yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitReason {
    /// Every slot it may take is held.
    Held,
    /// A slot is free but the host is under memory pressure.
    Pressure,
}

impl WaitReason {
    fn as_str(self) -> &'static str {
        match self {
            WaitReason::Held => "held",
            WaitReason::Pressure => "pressure",
        }
    }
}

/// What a waiter needs to decide whether memory pressure holds it back.
struct Admission<'a> {
    pressure: Option<&'a dyn Pressure>,
    remote: bool,
    pause: PressurePause,
    now: &'a dyn Fn() -> Instant,
}

impl Admission<'static> {
    /// No pressure source: never held back, never takes an extra.
    fn none() -> Self {
        Admission {
            pressure: None,
            remote: false,
            pause: PressurePause::new(PRESSURE_WAIT),
            now: &Instant::now,
        }
    }
}

/// One pressure reading per poll, taken only when something needs it.
struct Reading<'a> {
    pressure: Option<&'a dyn Pressure>,
    value: std::cell::Cell<Option<Option<bool>>>,
}

impl<'a> Reading<'a> {
    fn new(pressure: Option<&'a dyn Pressure>) -> Self {
        Self {
            pressure,
            value: std::cell::Cell::new(None),
        }
    }

    fn get(&self) -> Option<bool> {
        if let Some(value) = self.value.get() {
            return value;
        }
        let value = self.pressure.and_then(|pressure| pressure.high());
        self.value.set(Some(value));
        value
    }
}

/// Whether the waiter should skip a free base slot now: the host is under pressure, another
/// slot is held so waiting can help, and the waiter has not already paused for the cap.
fn base_held_back(
    dir: &Path,
    layout: &Layout,
    admission: &mut Admission<'_>,
    reading: &Reading<'_>,
    allowed: &dyn Fn(usize) -> bool,
) -> bool {
    if admission.pressure.is_none() || admission.pause.expired() {
        return false;
    }
    let free = (0..layout.count).any(|index| {
        allowed(index) && matches!(lock_slot(&slot_path(dir, index), true), Ok(Some(_)))
    });
    if !free || held(dir, layout.counted(), usize::MAX) <= 1 {
        return false;
    }
    let now = (admission.now)();
    admission.pause.hold_back(reading.get(), now)
}

/// One all-or-nothing try by a waiter that is not first in line. Takes shared
/// locks on its reserve and one base or extra slot, all outside `allowed`'s
/// refusals, and keeps nothing when any step fails.
fn pass_ahead(
    dir: &Path,
    layout: &Layout,
    admission: &mut Admission<'_>,
    reading: &Reading<'_>,
    label: &str,
    allowed: &dyn Fn(usize) -> bool,
    on_skip: &mut dyn FnMut(&Path, &io::Error),
) -> Option<Slot> {
    let mut held: Vec<File> = Vec::new();
    for index in layout.reserve.clone() {
        if !allowed(index) {
            return None;
        }
        let path = slot_path(dir, index);
        match lock_slot(&path, true) {
            Ok(Some(file)) => held.push(file),
            Ok(None) => return None,
            Err(error) => on_skip(&path, &error),
        }
    }
    if base_held_back(dir, layout, admission, reading, allowed) {
        return None;
    }
    let (taken, skipped) = scan(dir, 0..layout.count, label, allowed);
    for (path, error) in &skipped {
        on_skip(path, error);
    }
    let taken = taken.ok().flatten().or_else(|| {
        admission
            .remote
            .then(|| take_extra(dir, layout.extras.clone(), label, reading.get(), allowed))
            .flatten()
    });
    let mut slot = taken?;
    slot._reserve = held;
    Some(slot)
}

fn acquire_reserving(
    dir: &Path,
    layout: &Layout,
    mut admission: Admission<'_>,
    label: &str,
    on_wait: &mut dyn FnMut(&str, WaitReason),
    on_poll: &mut dyn FnMut(),
    on_skip: &mut dyn FnMut(&Path, &io::Error),
) -> io::Result<Slot> {
    let ticket = enqueue_wanting(dir, label, &Want::of(layout, admission.remote)).ok();
    let mut pending: Vec<usize> = layout.reserve.clone().collect();
    let mut held: Vec<File> = Vec::new();
    let mut announced = false;
    loop {
        let reading = Reading::new(admission.pressure);
        let mut held_back = false;
        let (ahead, forbidden) = ticket
            .as_ref()
            .map_or((0, Vec::new()), |ticket| ticket.ahead(dir));
        if ahead > 0 {
            // Pass waiters ahead only through slots none of them can use now.
            let allowed = |index: usize| !is_forbidden(&forbidden, index);
            let passed = pass_ahead(
                dir,
                layout,
                &mut admission,
                &reading,
                label,
                &allowed,
                on_skip,
            );
            if let Some(slot) = passed {
                return Ok(slot);
            }
        } else {
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
            held_back = pending.is_empty()
                && base_held_back(dir, layout, &mut admission, &reading, &|_| true);
            if pending.is_empty() && !held_back {
                let (taken, skipped) = scan(dir, 0..layout.count, label, &|_| true);
                let taken = taken?;
                for (path, error) in &skipped {
                    on_skip(path, error);
                }
                let taken = taken.or_else(|| {
                    admission.remote.then(|| {
                        take_extra(dir, layout.extras.clone(), label, reading.get(), &|_| true)
                    })?
                });
                if let Some(mut slot) = taken {
                    slot._reserve = held;
                    return Ok(slot);
                }
            }
        }
        if !announced {
            announced = true;
            let names = holder(dir, layout.named(), ahead);
            let extras_paused = admission.remote
                && reading.get() == Some(true)
                && extra_free(dir, layout.extras.clone());
            let (message, reason) = if held_back {
                (
                    format!("{names}; paused under memory pressure"),
                    WaitReason::Pressure,
                )
            } else if extras_paused {
                (
                    format!("{names}; extra slots paused under memory pressure"),
                    WaitReason::Pressure,
                )
            } else {
                (names, WaitReason::Held)
            };
            on_wait(&message, reason);
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

/// Reads `MemAvailable` from `/proc/meminfo` text, in bytes.
pub fn parse_mem_available(text: &str) -> Option<u64> {
    let line = text
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

/// Where this dx process takes its slots, fixed at startup.
#[derive(Clone, Debug)]
pub struct Gate {
    dir: PathBuf,
    count: usize,
    pool: usize,
    extras: usize,
    remote: bool,
    pressure: Arc<dyn Pressure>,
    clock: Clock,
    label: String,
    json: bool,
    warm: Option<Warmth>,
    skipped: Arc<Mutex<BTreeSet<PathBuf>>>,
    resources: Option<Resources>,
    reclaimer: Option<Reclaimer>,
    reaper: Option<Reaper>,
}

/// The time source a gate paces its pressure pause by.
#[derive(Clone)]
struct Clock(Arc<dyn Fn() -> Instant + Send + Sync>);

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clock")
    }
}

/// Stops idle Bazel servers outside the kept workspaces until about `target` bytes are freed; returns how many stopped.
#[derive(Clone)]
#[allow(clippy::type_complexity)]
struct Reaper(Arc<dyn Fn(&[PathBuf], u64) -> usize + Send + Sync>);

impl std::fmt::Debug for Reaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reaper")
    }
}

/// Frees disk before a build; returns a message only when it acted or failed.
#[derive(Clone)]
struct Reclaimer(Arc<dyn Fn() -> Option<String> + Send + Sync>);

impl std::fmt::Debug for Reclaimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reclaimer")
    }
}

/// The CPUs, memory, and nested reserve a gate splits over held slots.
#[derive(Clone, Copy, Debug)]
struct Resources {
    cpus: u64,
    memory: u64,
    reserved: u64,
}

impl Gate {
    /// A gate over `count` slots in `dir`. It reads no host pressure until
    /// [`Gate::with_pressure`]; [`Gate::from_env`] installs the host reader.
    pub fn new(dir: PathBuf, count: usize, label: String, json: bool) -> Self {
        Gate {
            dir,
            count,
            pool: count,
            extras: 0,
            remote: false,
            pressure: Arc::new(NoSignal),
            clock: Clock(Arc::new(Instant::now)),
            label,
            json,
            warm: None,
            skipped: Arc::default(),
            resources: None,
            reclaimer: None,
            reaper: None,
        }
    }

    /// Runs `reap` once per wait, before the wait notice, with the workspaces to keep and the bytes to free.
    pub fn reaping(self, reap: impl Fn(&[PathBuf], u64) -> usize + Send + Sync + 'static) -> Self {
        Gate {
            reaper: Some(Reaper(Arc::new(reap))),
            ..self
        }
    }

    /// Runs the reaper unless another waiter is reaping; `None` when the gate has no reaper.
    fn reap(&self, layout: &Layout) -> Option<usize> {
        let reaper = self.reaper.as_ref()?;
        let _ = std::fs::create_dir_all(&self.dir);
        let Ok(Some(_lock)) = dx_atomic_fs::lease::try_lock_file(&self.dir.join("reap.lock"), true)
        else {
            return Some(0);
        };
        let keep = kept_workspaces(&self.dir, layout.named(), &self.label);
        let target = if self.remote {
            BYTES_PER_REMOTE_SLOT
        } else {
            BYTES_PER_SLOT
        };
        Some((reaper.0)(&keep, target))
    }

    /// Runs `reclaim` each time a slot is taken, reporting any message it returns as a disk_low warning.
    pub fn reclaiming(self, reclaim: impl Fn() -> Option<String> + Send + Sync + 'static) -> Self {
        Gate {
            reclaimer: Some(Reclaimer(Arc::new(reclaim))),
            ..self
        }
    }

    /// Gives each taken slot a share of `cpus` and of `memory` bytes less `reserved`.
    pub fn sharing(self, cpus: u64, memory: u64, reserved: u64) -> Self {
        Gate {
            resources: Some(Resources {
                cpus,
                memory,
                reserved,
            }),
            ..self
        }
    }

    /// Spans `pool` slots, keeping those above `count` for nested Bazel servers.
    pub fn reserving(self, pool: usize) -> Self {
        Gate {
            pool: pool.max(self.count),
            ..self
        }
    }

    /// Adds `extras` remote-only slots above the pool; only a `remote` gate takes them.
    pub fn extra(self, extras: usize, remote: bool) -> Self {
        Gate {
            extras,
            remote,
            ..self
        }
    }

    /// Paces the pressure pause by `clock`.
    #[cfg(test)]
    fn with_clock(self, clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Gate {
            clock: Clock(Arc::new(clock)),
            ..self
        }
    }

    /// Reads memory pressure from `pressure` before taking a base or extra slot.
    pub fn with_pressure(self, pressure: Arc<dyn Pressure>) -> Self {
        Gate { pressure, ..self }
    }

    fn layout(&self) -> Layout {
        Layout {
            count: self.count,
            reserve: self.count..self.pool,
            extras: self.pool..self.pool + self.extras,
        }
    }

    /// The gate for this process; `None` when nested, inside a Bazel test, set `off`, or without a cache dir.
    pub fn from_env(
        workspace: &Path,
        env_get: &dyn Fn(&str) -> Option<String>,
        json: bool,
        bazel_options: &[String],
        setting: BuildSlots,
    ) -> Option<Self> {
        let set = |name: &str| env_get(name).is_some_and(|value| !value.is_empty());
        if set(SLOT_ENV_VAR) || set(BAZEL_TEST_ENV_VAR) {
            return None;
        }
        if setting == BuildSlots::Off {
            return None;
        }
        let dir = slots_dir(env_get)?;
        let memory = available_memory()?;
        let reserved = nested_reserve(workspace, env_get);
        let Sizing {
            count,
            pool,
            extras,
        } = sizing(setting, memory, reserved)?;
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        let read_user_rc = dx_process::RcPolicy::from_env(env_get, workspace, &read).read_user_rc;
        let remote = dx_process::rc::remote_execution(
            env_get,
            workspace,
            &read,
            read_user_rc,
            bazel_options,
        );
        let cpus = std::thread::available_parallelism().map_or(1, |cpus| cpus.get());
        let cpus = u64::try_from(cpus).unwrap_or(1);
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        let warm = own_startup_options(&args).map(|startup_options| Warmth {
            workspace: workspace.to_path_buf(),
            startup_options,
        });
        let mut roots = host::default_output_roots(env_get);
        if let Some(root) = warm.as_ref().and_then(|warm| {
            startup_value(&warm.startup_options, "--output_user_root").map(PathBuf::from)
        }) {
            roots.push(root);
        }
        let reclaim_in = workspace.to_path_buf();
        Some(Gate {
            warm,
            ..Gate::new(dir, count, workspace.display().to_string(), json)
                .reserving(pool)
                .extra(extras, remote)
                .with_pressure(Arc::new(HostPressure::new(if remote {
                    BYTES_PER_REMOTE_SLOT
                } else {
                    BYTES_PER_SLOT
                })))
                .sharing(cpus, memory, reserved)
                .reclaiming(move || dx_worktree::reclaim_for_build(&reclaim_in))
                .reaping(move |workspaces, target| {
                    let keep = dx_worktree::reap::Keep {
                        workspaces: workspaces.to_vec(),
                        roots: roots.clone(),
                    };
                    dx_worktree::reap::reap_idle(
                        &SystemHost,
                        &keep,
                        target,
                        REAP_MIN_IDLE.as_secs(),
                    )
                    .servers
                })
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

    /// Takes a slot, waiting with one notice when none is free, then reclaims disk; `Err` carries the stop signal.
    pub fn take(
        &self,
        command: Command,
        err: &mut dyn Write,
        events: &mut dyn Write,
    ) -> Result<Option<Slot>, i32> {
        let json = self.json;
        let mut keep = self.warm.clone().map(KeepWarm::new);
        let out = RefCell::new((err, events));
        let layout = self.layout();
        let admission = Admission {
            pressure: Some(&*self.pressure),
            remote: self.remote,
            pause: PressurePause::new(PRESSURE_WAIT),
            now: &*self.clock.0,
        };
        let result = acquire_reserving(
            &self.dir,
            &layout,
            admission,
            &self.label,
            &mut |holder: &str, reason: WaitReason| {
                let reaped = self.reap(&layout);
                let (err, events) = &mut *out.borrow_mut();
                let message = wait_message(holder) + &reaped_suffix(reaped.unwrap_or(0));
                let _ = writeln!(err, "dx: {message}");
                if json {
                    let mut event = notice_event(
                        command,
                        dx_output::NoticeEvent::info(WAITING_FOR_BUILD_SLOT_CODE, message),
                    );
                    if let Some(map) = event.as_object_mut() {
                        if let Some(reaped) = reaped {
                            map.insert("reaped_servers".to_owned(), reaped.into());
                        }
                        map.insert("reason".to_owned(), reason.as_str().into());
                    }
                    write_notice(&mut **events, &event);
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
                        dx_output::NoticeEvent::warning(BUILD_SLOT_SKIPPED_CODE, message),
                    );
                }
            },
        );
        drop(keep);
        let (err, events) = out.into_inner();
        match result {
            Ok(mut slot) => {
                if let Some(resources) = self.resources {
                    let sizing = Arc::new(ShareSizing {
                        dir: self.dir.clone(),
                        indices: layout.counted().collect(),
                        own: slot.index,
                        resources,
                    });
                    let live = Arc::clone(&sizing);
                    slot.token = Some(dx_process::set_share_source(move || live.share()));
                    slot.sizing = Some(sizing);
                }
                self.reclaim(command, err, events);
                Ok(Some(slot))
            }
            Err(error) => {
                if let Some(signo) = dx_process::interrupted() {
                    return Err(signo);
                }
                let message = format!("build slots disabled: {error}");
                let _ = writeln!(err, "dx: {message}");
                if json {
                    notice(
                        &mut *events,
                        command,
                        dx_output::NoticeEvent::warning(BUILD_SLOTS_DISABLED_CODE, message),
                    );
                }
                self.reclaim(command, err, events);
                Ok(None)
            }
        }
    }

    /// Runs the reclaimer, printing one line and, in JSON mode, one disk_low notice when it reports.
    fn reclaim(&self, command: Command, err: &mut dyn Write, events: &mut dyn Write) {
        let Some(message) = self
            .reclaimer
            .as_ref()
            .and_then(|reclaimer| (reclaimer.0)())
        else {
            return;
        };
        let _ = writeln!(err, "dx: {message}");
        if self.json {
            notice(
                events,
                command,
                dx_output::NoticeEvent::warning(DISK_LOW_CODE, message),
            );
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

/// How often a waiting run pings a server with this idle limit, or `None` when the
/// server never idles out (`--max_idle_secs=0`) and needs no pings.
fn ping_interval(max_idle_secs: Option<u64>) -> Option<Duration> {
    let idle = max_idle_secs.unwrap_or(10_800);
    if idle == 0 {
        return None;
    }
    Some(Duration::from_secs(idle / 3).clamp(Duration::from_secs(1), Duration::from_secs(60)))
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
            &host::default_output_roots(&|name| std::env::var(name).ok()),
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

    #[allow(clippy::disallowed_methods)] // info takes no build settings
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
                Some((pid, idle)) => match ping_interval(idle) {
                    Some(every) => Warming::On {
                        pid,
                        every,
                        due: now,
                        ping: None,
                    },
                    None => Warming::Off,
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

fn notice(events: &mut dyn Write, command: Command, notice: dx_output::NoticeEvent) {
    write_notice(events, &notice_event(command, notice));
}

fn notice_event(command: Command, notice: dx_output::NoticeEvent) -> serde_json::Value {
    dx_output::notice_value(&dx_output::NoticeEvent {
        related_command: Some(command.name().to_owned()),
        ..notice
    })
}

fn write_notice(events: &mut dyn Write, event: &serde_json::Value) {
    let _ = dx_output::lifecycle::write_event(&mut *events, event);
    let _ = events.flush();
}

/// The wait-line tail naming stopped servers; empty when none stopped.
fn reaped_suffix(reaped: usize) -> String {
    match reaped {
        0 => String::new(),
        1 => "; stopped 1 idle Bazel server".to_owned(),
        n => format!("; stopped {n} idle Bazel servers"),
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
fn take(
    command: Command,
    err: &mut dyn Write,
    events: &mut dyn Write,
) -> Result<Option<Slot>, i32> {
    match GATE.with(|cell| cell.borrow().clone()) {
        Some(gate) => gate.take(command, err, events),
        None => Ok(None),
    }
}

/// One phase's lazily taken slot: taken at most once, on the first need.
pub struct Phase {
    command: Command,
    asked: bool,
    slot: Option<Slot>,
}

impl Phase {
    /// Starts a phase for `command` holding nothing.
    pub fn new(command: Command) -> Self {
        Phase {
            command,
            asked: false,
            slot: None,
        }
    }

    /// Takes the slot on the first call with `needed`; never asks twice, even after none was granted.
    pub fn need(
        &mut self,
        needed: bool,
        err: &mut dyn Write,
        events: &mut dyn Write,
    ) -> Result<Option<&Slot>, i32> {
        if needed && !self.asked {
            self.asked = true;
            self.slot = take(self.command, err, events)?;
        }
        Ok(self.slot.as_ref())
    }

    /// The slot held now, if any.
    pub fn slot(&self) -> Option<&Slot> {
        self.slot.as_ref()
    }

    /// Frees the slot held now.
    pub fn release(&mut self) {
        self.slot = None;
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
const SETTLE_LIMIT: Duration = Duration::from_secs(1);
#[cfg(test)]
const SETTLE_POLL: Duration = Duration::from_millis(10);

/// Waits for `count` slots to look free, since a forked child can briefly hold a released slot's lock.
/// See `a_child_forked_while_a_slot_is_held_keeps_it_until_exec` for why.
#[cfg(test)]
fn slot_settles_free(dir: &Path, count: usize) -> bool {
    let start = Instant::now();
    loop {
        if try_take(dir, count, "settle").expect("try").is_some() {
            return true;
        }
        if start.elapsed() >= SETTLE_LIMIT {
            return false;
        }
        std::thread::sleep(SETTLE_POLL);
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
        Gate::from_env(workspace, env_get, json, &[], BuildSlots::Auto)
            .map(|gate| gate.with_pressure(Arc::new(Fixed(Some(false)))))
            .map_or(Ok(None), |gate| gate.hold(command, err, events))
    }

    const ALONE_ENV: &str = "DX_SLOT_TEST_ALONE";

    /// Runs `body` in a child test process so no other test touches the process share.
    fn alone(test: &str, body: impl FnOnce()) {
        if std::env::var_os(ALONE_ENV).is_some() {
            body();
            return;
        }
        let status = Process::new(std::env::current_exe().expect("exe"))
            .arg(format!("slots::tests::{test}"))
            .arg("--exact")
            .env(ALONE_ENV, "1")
            .stdout(Stdio::null())
            .status()
            .expect("spawn");
        assert!(status.success(), "{test} failed in its own process");
    }

    const HOLD_ENV: &str = "DX_SLOT_TEST_HOLD";

    #[test]
    fn slot_holder_helper() {
        let Ok(dir) = std::env::var(HOLD_ENV) else {
            return;
        };
        let _slot = acquire(Path::new(&dir), 1, "helper-tree", &mut |_| {}).expect("slot");
        std::fs::write(Path::new(&dir).join("held"), b"1").expect("marker");
        std::thread::sleep(Duration::from_secs(120));
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

    #[test]
    fn remote_count_follows_memory_with_bounds() {
        assert_eq!(remote_slot_count(0, 0), 1);
        assert_eq!(remote_slot_count(19 << 30, 0), 12);
        assert_eq!(remote_slot_count(19 << 30, 6 << 30), 8);
        assert_eq!(remote_slot_count(u64::MAX, 0), MAX_SLOTS);
        let extras = |memory, reserved| {
            remote_slot_count(memory, reserved).saturating_sub(slot_count(memory, reserved))
        };
        assert_eq!(extras(19 << 30, 0), 6);
        assert_eq!(extras(19 << 30, 6 << 30), 4);
        assert_eq!(extras(2 << 30, 0), 0);
    }

    #[test]
    fn extras_sit_above_the_pool() {
        let rules_dx = Gate::new(PathBuf::from("/slots"), 4, "r".to_owned(), false)
            .reserving(6)
            .extra(4, true)
            .layout();
        assert_eq!(
            (
                rules_dx.count,
                rules_dx.reserve.clone(),
                rules_dx.extras.clone()
            ),
            (4, 4..6, 6..10)
        );
        assert_eq!(
            rules_dx.counted().collect::<Vec<_>>(),
            [0, 1, 2, 3, 6, 7, 8, 9]
        );
        let consumer = Gate::new(PathBuf::from("/slots"), 6, "c".to_owned(), false)
            .reserving(6)
            .extra(6, false)
            .layout();
        assert_eq!(
            (consumer.count, consumer.reserve, consumer.extras),
            (6, 6..6, 6..12)
        );
    }

    #[derive(Debug)]
    struct Fixed(Option<bool>);

    impl Pressure for Fixed {
        fn high(&self) -> Option<bool> {
            self.0
        }
    }

    fn remote_gate(dir: &Path, remote: bool, high: Option<bool>) -> Gate {
        Gate::new(dir.to_path_buf(), 1, "remote".to_owned(), false)
            .extra(2, remote)
            .with_pressure(Arc::new(Fixed(high)))
    }

    fn free(dir: &Path, index: usize) -> bool {
        matches!(lock_slot(&slot_path(dir, index), false), Ok(Some(_)))
    }

    #[test]
    fn a_local_gate_never_takes_an_extra() {
        let scratch = dx_test_scratch::scratch("dx-slots-local-extra-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        let gate = remote_gate(&dir, false, Some(false));
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(free(&dir, 1) && free(&dir, 2));
        drop(fill);
        assert_eq!(waiter.join().expect("join").0.index, 0);
    }

    #[test]
    fn a_remote_gate_takes_a_base_slot_then_an_extra_when_calm() {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-extra-");
        let dir = scratch.path().join("slots");
        let gate = remote_gate(&dir, true, Some(false));
        let (base, base_err, _) = take_gate(&gate);
        let (extra, extra_err, _) = take_gate(&gate);
        assert_eq!((base.index, extra.index), (0, 1));
        assert_eq!((base_err.as_str(), extra_err.as_str()), ("", ""));
        assert!(extra_mark(&dir).is_file());
    }

    #[test]
    fn a_remote_gate_waits_under_pressure_or_without_a_signal() {
        for high in [Some(true), None] {
            let scratch = dx_test_scratch::scratch("dx-slots-remote-pressure-");
            let dir = scratch.path().join("slots");
            let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
            let gate = remote_gate(&dir, true, high);
            let (waiter, rx) = spawn_take(gate);
            announced(&rx);
            std::thread::sleep(POLL * 2);
            assert!(free(&dir, 1) && free(&dir, 2));
            drop(fill);
            let (slot, err, _) = waiter.join().expect("join");
            assert_eq!(slot.index, 0);
            let want = match high {
                Some(true) => "dx: waiting for a build slot (held by fill; extra slots paused under memory pressure)\n",
                _ => "dx: waiting for a build slot (held by fill)\n",
            };
            assert_eq!(err, want);
        }
    }

    #[test]
    fn a_remote_waiter_does_not_jump_the_line_through_a_free_extra() {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-line-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        let queued = enqueue(&dir, "").expect("queued");
        let gate = remote_gate(&dir, true, Some(false));
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(!waiter.is_finished(), "a remote waiter must keep its place");
        assert!(free(&dir, 1) && free(&dir, 2));
        drop(queued);
        let (slot, err, _) = waiter.join().expect("join");
        assert_eq!(slot.index, 1);
        assert_eq!(
            err,
            "dx: waiting for a build slot (held by fill, 1 ahead in line)\n"
        );
        drop(fill);
    }

    #[test]
    fn held_extras_under_pressure_add_no_paused_suffix() {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-full-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        let extras: Vec<Slot> = (1..3)
            .map(|index| {
                scan(&dir, index..index + 1, &format!("extra-{index}"), &|_| true)
                    .0
                    .expect("scan")
                    .expect("extra")
            })
            .collect();
        // Each clock read is past the cap, so the second read releases the pause.
        let start = Instant::now();
        let ticks = std::sync::atomic::AtomicU32::new(0);
        let clock = move || {
            let n = ticks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            start + PRESSURE_WAIT * (n + 1)
        };
        let gate = remote_gate(&dir, true, Some(true)).with_clock(clock);
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(fill);
        let (slot, err, _) = waiter.join().expect("join");
        assert_eq!(slot.index, 0);
        assert_eq!(
            err,
            "dx: waiting for a build slot (held by fill and 2 more)\n"
        );
        drop(extras);
    }

    #[derive(Debug)]
    struct Switch(std::sync::atomic::AtomicBool);

    impl Pressure for Switch {
        fn high(&self) -> Option<bool> {
            Some(self.0.load(std::sync::atomic::Ordering::SeqCst))
        }
    }

    /// A clock that starts now and moves only when told.
    fn fake_clock() -> (
        impl Fn() -> Instant + Send + Sync + 'static,
        Arc<Mutex<Duration>>,
    ) {
        let start = Instant::now();
        let offset = Arc::new(Mutex::new(Duration::ZERO));
        let read = offset.clone();
        (move || start + *read.lock().expect("clock"), offset)
    }

    #[test]
    fn a_new_gate_reads_no_host_pressure() {
        let gate = Gate::new(PathBuf::from("/slots"), 1, "g".to_owned(), false);
        assert_eq!(gate.pressure.high(), None);
    }

    fn base_gate(dir: &Path, high: Arc<dyn Pressure>) -> Gate {
        Gate::new(dir.to_path_buf(), 2, "gate".to_owned(), true).with_pressure(high)
    }

    #[test]
    fn a_free_base_slot_waits_under_pressure_while_another_is_held() {
        let scratch = dx_test_scratch::scratch("dx-slots-base-pressure-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
        let switch = Arc::new(Switch(std::sync::atomic::AtomicBool::new(true)));
        let (waiter, rx) = spawn_take(base_gate(&dir, switch.clone()));
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(!waiter.is_finished());
        switch.0.store(false, std::sync::atomic::Ordering::SeqCst);
        let (slot, err, events) = waiter.join().expect("join");
        assert_eq!(slot.index, 1);
        assert_eq!(
            err,
            "dx: waiting for a build slot (held by fill; paused under memory pressure)\n"
        );
        assert!(events.contains("\"reason\":\"pressure\""), "{events}");
        drop(fill);
    }

    #[test]
    fn the_pause_ends_after_the_cap_even_under_pressure() {
        let scratch = dx_test_scratch::scratch("dx-slots-base-cap-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
        let (clock, offset) = fake_clock();
        let gate = base_gate(&dir, Arc::new(Fixed(Some(true)))).with_clock(clock);
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(!waiter.is_finished());
        *offset.lock().expect("clock") = PRESSURE_WAIT;
        let (slot, _, _) = waiter.join().expect("join");
        assert_eq!(slot.index, 1);
        drop(fill);
    }

    #[test]
    fn nothing_pauses_when_no_other_slot_is_held_or_there_is_no_signal() {
        let scratch = dx_test_scratch::scratch("dx-slots-base-free-");
        let dir = scratch.path().join("slots");
        let gate = base_gate(&dir, Arc::new(Fixed(Some(true))));
        let (slot, err, _) = take_gate(&gate);
        assert_eq!((slot.index, err.as_str()), (0, ""));
        let blind = base_gate(&dir, Arc::new(Fixed(None)));
        let (other, err, _) = take_gate(&blind);
        assert_eq!((other.index, err.as_str()), (1, ""));
    }

    #[test]
    fn a_normal_wait_reports_the_held_reason() {
        let scratch = dx_test_scratch::scratch("dx-slots-base-held-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        let gate = Gate::new(dir.clone(), 1, "gate".to_owned(), true)
            .with_pressure(Arc::new(Fixed(Some(false))));
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(fill);
        let (_, _, events) = waiter.join().expect("join");
        assert!(events.contains("\"reason\":\"held\""), "{events}");
    }

    #[test]
    fn a_local_rules_dx_gate_counts_a_held_extra_but_not_the_reserve() {
        let scratch = dx_test_scratch::scratch("dx-slots-local-counted-");
        let dir = scratch.path().join("slots");
        let layout = Gate::new(dir.clone(), 4, "r".to_owned(), false)
            .reserving(6)
            .extra(4, false)
            .layout();
        let take = |index: usize| {
            scan(&dir, index..index + 1, "other", &|_| true)
                .0
                .expect("scan")
                .expect("slot")
        };
        let reserve = take(4);
        assert_eq!(held(&dir, layout.counted(), 0), 1);
        let extra = take(6);
        assert_eq!(held(&dir, layout.counted(), 0), 2);
        let base = take(1);
        assert_eq!(held(&dir, layout.counted(), 0), 3);
        drop((reserve, extra, base));
    }

    #[test]
    fn extra_grants_are_spaced_out() {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-spacing-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        std::fs::write(extra_mark(&dir), "earlier").expect("mark");
        let gate = remote_gate(&dir, true, Some(false));
        let waiting = gate.clone();
        let (waiter, rx) = spawn_take(waiting);
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(free(&dir, 1) && free(&dir, 2));
        drop(fill);
        let base = waiter.join().expect("join").0;
        assert_eq!(base.index, 0);
        let old = SystemTime::now() - EXTRA_SPACING - Duration::from_secs(1);
        File::options()
            .write(true)
            .open(extra_mark(&dir))
            .and_then(|file| file.set_modified(old))
            .expect("age mark");
        let (extra, err, _) = take_gate(&gate);
        assert_eq!((extra.index, err.as_str()), (1, ""));
    }

    #[test]
    fn a_held_extra_counts_toward_the_share() {
        alone("a_held_extra_counts_toward_the_share", || {
            let scratch = dx_test_scratch::scratch("dx-slots-remote-share-");
            let dir = scratch.path().join("slots");
            let gate = remote_gate(&dir, true, Some(false)).sharing(12, 12 << 30, 0);
            let (base, _, _) = take_gate(&gate);
            let (extra, _, _) = take_gate(&gate);
            assert_eq!(extra.index, 1);
            let share = extra.share().expect("share");
            assert_eq!((share.cpus, share.memory_mb), (6, 6 << 10));
            drop(base);
            let local = remote_gate(&dir, false, None).sharing(12, 12 << 30, 0);
            let (again, _, _) = take_gate(&local);
            assert_eq!(again.index, 0);
            assert_eq!(again.share().expect("share").memory_mb, 6 << 10);
        });
    }

    #[test]
    fn share_splits_the_host_over_held_slots() {
        let share = |cpus, memory, reserved, held| {
            let share = local_share(cpus, memory, reserved, held);
            (share.cpus, share.memory_mb)
        };
        assert_eq!(share(12, 19 << 30, 0, 1), (12, 19 << 10));
        assert_eq!(share(12, 19 << 30, 0, 2), (6, 9728));
        assert_eq!(share(12, 19 << 30, 0, 4), (3, 4864));
        assert_eq!(share(4, 8 << 30, 0, 4), (2, 2048));
        assert_eq!(share(1, 8 << 30, 0, 1), (2, 8192));
        assert_eq!(share(12, 19 << 30, 6 << 30, 2), (6, 6656));
        assert_eq!(share(12, 4 << 30, 6 << 30, 2), (6, 1));
        assert_eq!(share(12, 19 << 30, 0, 0), (12, 19 << 10));
    }

    #[cfg(unix)]
    #[test]
    fn a_child_forked_while_a_slot_is_held_keeps_it_until_exec() {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;
        use std::os::unix::process::CommandExt;
        let scratch = dx_test_scratch::scratch("dx-slots-fork-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("dir");
        let slot = try_take(&dir, 1, "held").expect("try").expect("free");
        let (mut reader, writer) = UnixStream::pair().expect("pair");
        let fd = writer.as_raw_fd();
        let spawner = std::thread::spawn(move || {
            let mut cmd = Process::new("/bin/sh");
            cmd.args(["-c", "exit 0"]);
            // SAFETY: the hook only calls write and nanosleep, both async-signal-safe.
            unsafe {
                cmd.pre_exec(move || {
                    let byte = 1u8;
                    libc::write(fd, (&byte as *const u8).cast(), 1);
                    let nap = libc::timespec {
                        tv_sec: 0,
                        tv_nsec: 150_000_000,
                    };
                    libc::nanosleep(&nap, std::ptr::null_mut());
                    Ok(())
                });
            }
            let mut child = cmd.spawn().expect("spawn");
            drop(writer);
            child.wait().expect("wait");
        });
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).expect("forked");
        drop(slot);
        assert!(
            try_take(&dir, 1, "probe").expect("try").is_none(),
            "the forked child holds the lock until it execs"
        );
        assert!(slot_settles_free(&dir, 1));
        spawner.join().expect("spawner");
    }

    #[test]
    fn held_counts_locked_slots_and_the_caller() {
        let scratch = dx_test_scratch::scratch("dx-slots-held-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(held(&dir, 0..3, 0), 1);
        let mut child = spawn_holder(&dir);
        assert_eq!(held(&dir, 0..3, 1), 2);
        assert_eq!(held(&dir, 0..3, 0), 1);
        let _ = child.kill();
        let _ = child.wait();
        std::fs::write(holder_path(&dir, 2), "gone").expect("stale holder");
        assert!(holder_path(&dir, 0).exists());
        assert_eq!(held(&dir, 0..3, 1), 1);
        assert!(slot_settles_free(&dir, 1));
    }

    #[test]
    fn a_taken_slot_sets_the_share_until_it_drops() {
        alone(
            "a_taken_slot_sets_the_share_until_it_drops",
            take_sets_the_share,
        );
    }

    fn take_sets_the_share() {
        let scratch = dx_test_scratch::scratch("dx-slots-take-share-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(600, 9 << 30, 1 << 30);
        let other = try_take(&dir, 2, "other").expect("try").expect("other");
        let (slot, _, _) = take_gate(&gate);
        let want = dx_process::LocalShare {
            cpus: 300,
            memory_mb: 4096,
        };
        assert_eq!(slot.share(), Some(want));
        assert_eq!(dx_process::share(), Some(want));
        drop(slot);
        assert_eq!(dx_process::share(), None);
        drop(other);
        let plain = Gate::new(dir, 2, "plain".to_owned(), false);
        let (slot, _, _) = take_gate(&plain);
        assert_eq!(slot.share(), None);
        assert_eq!(dx_process::share(), None);
    }

    #[test]
    fn share_shrinks_when_another_slot_is_taken() {
        alone("share_shrinks_when_another_slot_is_taken", || {
            let scratch = dx_test_scratch::scratch("dx-slots-live-share-");
            let dir = scratch.path().join("slots");
            let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(12, 12 << 30, 0);
            let (slot, _, _) = take_gate(&gate);
            let cpus = || dx_process::share().map(|share| share.cpus);
            assert_eq!(cpus(), Some(12));
            let other = try_take(&dir, 2, "other").expect("try").expect("other");
            assert_eq!(cpus(), Some(6));
            drop(other);
            assert_eq!(cpus(), Some(12));
            drop(slot);
            assert_eq!(cpus(), None);
        });
    }

    #[test]
    fn slot_runner_exports_the_current_share() {
        use dx_process::Runner as _;
        alone("slot_runner_exports_the_current_share", || {
            let scratch = dx_test_scratch::scratch("dx-slots-live-env-");
            let dir = scratch.path().join("slots");
            let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(12, 12 << 30, 0);
            let (slot, _, _) = take_gate(&gate);
            let inner = EnvRecorder::default();
            let held = slot.runner(&inner);
            held.run(&["bazel".to_owned()], scratch.path(), &[])
                .expect("run");
            let other = try_take(&dir, 2, "other").expect("try").expect("other");
            held.run(&["bazel".to_owned()], scratch.path(), &[])
                .expect("run");
            drop(other);
            let shares: Vec<String> = inner
                .seen
                .borrow()
                .iter()
                .flat_map(|(_, env)| env.iter())
                .filter(|(name, _)| name == SHARE_ENV_VAR)
                .map(|(_, value)| value.clone())
                .collect();
            assert_eq!(shares, vec!["cpu=12,memory=12288", "cpu=6,memory=6144"]);
        });
    }

    #[test]
    fn a_nested_call_reuses_the_parent_share() {
        alone(
            "a_nested_call_reuses_the_parent_share",
            nested_reuses_the_share,
        );
    }

    fn nested_reuses_the_share() {
        let env = |slot: Option<&'static str>, share: &'static str| {
            move |name: &str| match name {
                SLOT_ENV_VAR => slot.map(str::to_owned),
                SHARE_ENV_VAR => Some(share.to_owned()),
                _ => None,
            }
        };
        dx_process::set_share(None);
        inherit_share(&env(None, "cpu=5,memory=700"));
        assert_eq!(dx_process::share(), None);
        inherit_share(&env(Some("1"), "cpu=5,memory=bad"));
        assert_eq!(dx_process::share(), None);
        inherit_share(&env(Some("1"), "cpu=5,memory=700"));
        let want = dx_process::LocalShare {
            cpus: 5,
            memory_mb: 700,
        };
        assert_eq!(dx_process::share(), Some(want));
        dx_process::clear_share(&want);
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
        let gate =
            Gate::from_env(scratch.path(), &get, false, &[], BuildSlots::Auto).expect("gate");
        let resources = gate.resources.expect("resources");
        let cpus = std::thread::available_parallelism().expect("cpus").get();
        assert_eq!(resources.cpus, u64::try_from(cpus).expect("u64"));
        assert_eq!((resources.memory, resources.reserved), (memory, 6 << 30));
        assert_eq!(gate.count, slot_count(memory, 6 << 30));
        assert_eq!(gate.pool, slot_count(memory, 0));
        let consumer =
            Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).expect("gate");
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
            move |name: &str| (name == NESTED_BAZEL_SLOTS_ENV).then(|| value.to_owned())
        };
        assert_eq!(nested_reserve(scratch.path(), &with("5")), 10 << 30);
        for fallback in ["0", "x", ""] {
            assert_eq!(nested_reserve(scratch.path(), &with(fallback)), 6 << 30);
        }
    }

    #[test]
    fn auto_gate_reserve_is_the_shared_reserve_range() {
        for gib in [4_u64, 8, 12, 16, 24, 64] {
            let memory = gib << 30;
            let reserved = u64::from(DEFAULT_NESTED_BAZEL_SLOTS as u16) * BYTES_PER_NESTED_BAZEL;
            let size = sizing(BuildSlots::Auto, memory, reserved).expect("sizing");
            let range = dx_process::slot_pool::reserve_range(memory, DEFAULT_NESTED_BAZEL_SLOTS);
            assert_eq!(size.count..size.pool, range, "{gib} GiB");
        }
    }

    #[test]
    fn a_consumer_gate_leaves_the_held_reserve_alone() {
        let scratch = dx_test_scratch::scratch("dx-slots-reserve-held-");
        let dir = scratch.path().join("dx").join("slots");
        let memory = 16_u64 << 30;
        let reserve = dx_process::slot_pool::reserve_range(memory, DEFAULT_NESTED_BAZEL_SLOTS);
        let held = dx_process::slot_pool::hold_reserve(&dir, reserve.clone());
        assert!(held.busy.is_empty());
        let count = reserve.start;
        let slots: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("slot"))
            .collect();
        assert!(slots.iter().all(|slot| slot.index < count));
        for index in reserve {
            assert!(!free(&dir, index), "reserve index {index} is held");
        }
    }

    #[test]
    fn available_memory_text_parses() {
        let text = "MemTotal: 4096 kB\nMemAvailable:    2048 kB\n";
        assert_eq!(parse_mem_available(text), Some(2 << 20));
        assert_eq!(parse_mem_available("MemAvailable: x kB\n"), None);
        assert_eq!(parse_mem_available("MemTotal: 1 kB\n"), None);
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
        assert_eq!(parse_cgroup_limit("9223372036854771712\n"), None);
        assert_eq!(parse_cgroup_limit("9223372036854710272\n"), None);
        assert_eq!(parse_cgroup_limit("-1\n"), None);
        assert_eq!(parse_cgroup_limit("2147483648\n"), Some(2 << 30));
    }

    #[test]
    fn extras_follow_the_cgroup_limit() {
        let scratch = dx_test_scratch::scratch("dx-cgroup-extras-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("a")).expect("dirs");
        std::fs::write(root.join("a/memory.max"), format!("{}\n", 6u64 << 30)).expect("write");
        let host = 19u64 << 30;
        let limit = cgroup_limit_in("0::/a\n", root).expect("limit");
        let memory = host.min(limit);
        let extras = remote_slot_count(memory, 0).saturating_sub(slot_count(memory, 0));
        let host_extras = remote_slot_count(host, 0).saturating_sub(slot_count(host, 0));
        assert!(extras < host_extras);
        assert_eq!(extras, 2);
    }

    #[test]
    fn cgroup_limit_reads_v1_and_v2() {
        let put = |root: &Path, rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).expect("dirs");
            std::fs::write(path, text).expect("write");
        };
        let scratch = dx_test_scratch::scratch("dx-cgroup-v2-");
        let root = scratch.path();
        put(root, "a/memory.max", "1000\n");
        std::fs::create_dir_all(root.join("a/b")).expect("dirs");
        assert_eq!(cgroup_limit_in("0::/a/b\n", root), Some(1000));
        assert_eq!(cgroup_limit_in("", root), None);

        let scratch = dx_test_scratch::scratch("dx-cgroup-v1-");
        let root = scratch.path();
        put(root, "memory/memory.limit_in_bytes", "2000\n");
        assert_eq!(cgroup_limit_in("4:memory:/docker/x\n", root), Some(2000));
        assert_eq!(cgroup_limit_in("5:cpu,memory:/x\n", root), Some(2000));
        assert_eq!(cgroup_limit_in("5:cpu:/x\n", root), None);
        put(root, "a/memory.max", "3000\n");
        let hybrid = "4:memory:/docker/x\n0::/a\n";
        assert_eq!(cgroup_limit_in(hybrid, root), Some(2000));
        put(root, "a/memory.max", "1500\n");
        assert_eq!(cgroup_limit_in(hybrid, root), Some(1500));
        put(root, "memory/p/c/memory.limit_in_bytes", "5000\n");
        put(root, "memory/p/memory.limit_in_bytes", "700\n");
        assert_eq!(cgroup_limit_in("4:memory:/p/c\n", root), Some(700));

        let scratch = dx_test_scratch::scratch("dx-cgroup-v1-unlimited-");
        let root = scratch.path();
        put(
            root,
            "memory/memory.limit_in_bytes",
            "9223372036854771712\n",
        );
        assert_eq!(cgroup_limit_in("4:memory:/x\n", root), None);
    }

    #[derive(Default)]
    struct EnvRecorder {
        seen: RefCell<Vec<(&'static str, Vec<(String, String)>)>>,
    }

    impl EnvRecorder {
        fn record(&self, method: &'static str, env: &[(&str, &str)]) {
            let env = env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect();
            self.seen.borrow_mut().push((method, env));
        }
    }

    impl dx_process::Runner for EnvRecorder {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.record("run", env);
            Ok(dx_process::ChildStatus::exited(0))
        }

        fn run_hermetic(
            &self,
            _argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.record("run_hermetic", env);
            Ok(dx_process::ChildStatus::exited(3))
        }

        fn run_withdrawable(
            &self,
            _argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
            _withdraw: &dyn Fn() -> bool,
        ) -> io::Result<Option<dx_process::ChildStatus>> {
            self.record("run_withdrawable", env);
            Ok(Some(dx_process::ChildStatus::exited(4)))
        }

        fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
            Some(9)
        }
    }

    #[test]
    fn slot_runner_marks_children_while_the_slot_is_held() {
        use dx_process::Runner as _;
        let scratch = dx_test_scratch::scratch("dx-slots-env-");
        let dir = scratch.path().join("slots");
        let slot = try_take(&dir, 1, "env").expect("try").expect("free");
        let inner = EnvRecorder::default();
        let held = slot.runner(&inner);
        let argv = ["bazel".to_owned()];
        let cwd = scratch.path();
        let env = [("KEEP", "yes")];
        assert_eq!(held.run(&argv, cwd, &env).expect("run").code, Some(0));
        assert_eq!(
            held.run_withdrawable(&argv, cwd, &env, &|| true)
                .expect("withdrawable")
                .map(|status| status.code),
            Some(Some(4))
        );
        assert_eq!(
            held.run_hermetic(&argv, cwd, &env).expect("hermetic").code,
            Some(3)
        );
        assert_eq!(held.free_disk_bytes(cwd), Some(9));
        let marked = vec![
            ("KEEP".to_owned(), "yes".to_owned()),
            (SLOT_ENV_VAR.to_owned(), "1".to_owned()),
        ];
        let plain = vec![("KEEP".to_owned(), "yes".to_owned())];
        assert_eq!(
            *inner.seen.borrow(),
            vec![
                ("run", marked.clone()),
                ("run_withdrawable", marked),
                ("run_hermetic", plain),
            ]
        );
    }

    #[test]
    fn only_bazel_commands_take_slots() {
        for command in [Command::Build, Command::Test, Command::Coverage] {
            assert!(needs_slot(command));
        }
        for command in [Command::Run, Command::Watch, Command::Update] {
            assert!(!needs_slot(command));
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
            Command::Explain,
        ] {
            assert!(!needs_slot(command));
        }
    }

    #[test]
    fn slot_runner_passes_the_share_to_children() {
        use dx_process::Runner as _;
        let scratch = dx_test_scratch::scratch("dx-slots-share-env-");
        let dir = scratch.path().join("slots");
        let mut slot = try_take(&dir, 1, "env").expect("try").expect("free");
        slot.sizing = Some(Arc::new(ShareSizing {
            dir: dir.clone(),
            indices: vec![0],
            own: 0,
            resources: Resources {
                cpus: 7,
                memory: 777 << 20,
                reserved: 0,
            },
        }));
        let inner = EnvRecorder::default();
        let held = slot.runner(&inner);
        held.run(&["bazel".to_owned()], scratch.path(), &[])
            .expect("run");
        assert_eq!(
            *inner.seen.borrow(),
            vec![(
                "run",
                vec![
                    (SLOT_ENV_VAR.to_owned(), "1".to_owned()),
                    (SHARE_ENV_VAR.to_owned(), "cpu=7,memory=777".to_owned()),
                ]
            )]
        );
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
        let gate =
            Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).expect("gate");
        nested.set(true);
        assert!(Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).is_none());
        let slot = with_gate(Some(gate), || {
            take(Command::Run, &mut Vec::new(), &mut Vec::new())
        });
        assert!(matches!(
            slot,
            Ok(Some(Slot {
                sizing: Some(_),
                ..
            }))
        ));
        assert!(scratch
            .path()
            .join("dx")
            .join("slots")
            .join("slot-0.lock")
            .exists());
    }

    fn counting(
        message: Option<&'static str>,
    ) -> (
        Arc<std::sync::atomic::AtomicUsize>,
        impl Fn() -> Option<String> + Send + Sync + 'static,
    ) {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let reclaim = move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            message.map(str::to_owned)
        };
        (calls, reclaim)
    }

    const RECLAIMED: &str =
        "low disk: 5.0 GiB free; removed the output bases of free slot(s) 2; 30.0 GiB free now";

    #[test]
    fn take_runs_the_reclaimer_once_and_reports_it() {
        for json in [true, false] {
            let scratch = dx_test_scratch::scratch("dx-slots-reclaim-");
            let (calls, reclaim) = counting(Some(RECLAIMED));
            let gate = Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), json)
                .reclaiming(reclaim);
            let (_slot, err, events) = take_gate(&gate);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert_eq!(err, format!("dx: {RECLAIMED}\n"));
            if !json {
                assert!(events.is_empty(), "text mode writes no events");
                continue;
            }
            let notices: Vec<serde_json::Value> = events
                .lines()
                .map(|line| serde_json::from_str(line).expect("json"))
                .collect();
            assert_eq!(notices.len(), 1, "{events}");
            assert_eq!(notices[0]["event"], "notice");
            assert_eq!(notices[0]["level"], "warning");
            assert_eq!(notices[0]["code"], "disk_low");
            assert_eq!(notices[0]["message"], RECLAIMED);
            assert_eq!(notices[0]["related_command"], "test");
        }
    }

    #[test]
    fn hold_for_a_command_without_a_build_slot_never_reclaims() {
        let scratch = dx_test_scratch::scratch("dx-slots-no-reclaim-");
        let (calls, reclaim) = counting(Some(RECLAIMED));
        let gate =
            Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), true).reclaiming(reclaim);
        for command in [
            Command::Worktree,
            Command::Status,
            Command::Clean,
            Command::Watch,
        ] {
            let mut err = Vec::new();
            let mut events = Vec::new();
            let slot = gate.hold(command, &mut err, &mut events).expect("hold");
            assert!(slot.is_none(), "{command:?}");
            assert!(err.is_empty() && events.is_empty(), "{command:?}");
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        let mut err = Vec::new();
        let slot = gate
            .hold(Command::Build, &mut err, &mut Vec::new())
            .expect("hold");
        assert!(slot.is_some());
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "a build reclaims"
        );
    }

    #[test]
    fn a_quiet_reclaim_prints_nothing() {
        let scratch = dx_test_scratch::scratch("dx-slots-quiet-reclaim-");
        let (calls, reclaim) = counting(None);
        let gate =
            Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), true).reclaiming(reclaim);
        let (_slot, err, events) = take_gate(&gate);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(err.is_empty(), "{err}");
        assert!(events.is_empty(), "{events}");
    }

    #[test]
    fn take_reclaims_when_slots_are_disabled() {
        let scratch = dx_test_scratch::scratch("dx-slots-disabled-reclaim-");
        let blocker = scratch.path().join("slots");
        std::fs::write(&blocker, b"").expect("blocker");
        let (calls, reclaim) = counting(Some(RECLAIMED));
        let gate = Gate::new(blocker, 1, "me".to_owned(), false).reclaiming(reclaim);
        let mut err = Vec::new();
        let slot = gate
            .take(Command::Test, &mut err, &mut Vec::new())
            .expect("take");
        assert!(slot.is_none());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let text = String::from_utf8(err).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].starts_with("dx: build slots disabled: "), "{text}");
        assert_eq!(lines[1], format!("dx: {RECLAIMED}"));
    }

    #[test]
    fn a_nested_take_without_a_gate_never_reclaims() {
        let scratch = dx_test_scratch::scratch("dx-slots-nested-reclaim-");
        let (calls, reclaim) = counting(Some(RECLAIMED));
        let gate =
            Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), false).reclaiming(reclaim);
        with_gate(Some(gate), || {
            with_gate(None, || {
                take(Command::Build, &mut Vec::new(), &mut Vec::new()).expect("take")
            })
        });
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
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
        assert!(slot_settles_free(&dir, 1));
    }

    #[test]
    fn a_phase_takes_nothing_until_needed() {
        let scratch = dx_test_scratch::scratch("dx-slots-phase-lazy-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
        with_gate(Some(gate), || {
            let mut phase = Phase::new(Command::Run);
            let (mut err, mut events) = (Vec::new(), Vec::new());
            assert!(phase.need(false, &mut err, &mut events).unwrap().is_none());
            assert!(phase.slot().is_none());
            assert!(slot_settles_free(&dir, 1));
        });
    }

    #[test]
    fn a_phase_takes_once() {
        let scratch = dx_test_scratch::scratch("dx-slots-phase-once-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir, 1, "me".to_owned(), false);
        with_gate(Some(gate), || {
            let mut phase = Phase::new(Command::Run);
            let (mut err, mut events) = (Vec::new(), Vec::new());
            assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
            assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
            assert!(err.is_empty());
        });
    }

    #[test]
    fn a_phase_without_a_gate_holds_nothing() {
        let mut phase = Phase::new(Command::Run);
        let (mut err, mut events) = (Vec::new(), Vec::new());
        assert!(phase.need(true, &mut err, &mut events).unwrap().is_none());
        assert!(phase.need(true, &mut err, &mut events).unwrap().is_none());
        assert!(phase.slot().is_none());
    }

    #[test]
    fn a_released_phase_frees_its_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-phase-release-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
        with_gate(Some(gate), || {
            let mut phase = Phase::new(Command::Run);
            let (mut err, mut events) = (Vec::new(), Vec::new());
            assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
            phase.release();
            assert!(phase.slot().is_none());
            assert!(slot_settles_free(&dir, 1));
        });
    }

    #[test]
    fn share_report_counts_held_slots_read_only() {
        let scratch = dx_test_scratch::scratch("dx-slots-report-");
        let cache = scratch.path().display().to_string();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let dir = slots_dir(&get).expect("dir");
        std::fs::create_dir_all(&dir).expect("dir");
        let report = |get: &dyn Fn(&str) -> Option<String>| {
            share_report(
                scratch.path(),
                get,
                BuildSlots::Count(2.try_into().expect("two")),
            )
        };
        let idle = report(&get).expect("report");
        assert_eq!((idle.held, idle.count), (0, 2));
        let _held = try_take(&dir, 2, "me").expect("try").expect("slot");
        std::fs::write(holder_path(&dir, 0), "me").expect("holder");
        let busy = report(&get).expect("report");
        assert_eq!(busy.held, 1);
        assert!(busy.next.cpus <= idle.next.cpus);
        assert!(!slot_path(&dir, 1).exists(), "the probe creates no slot");
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
        let local = |name: &str| (name == "LOCALAPPDATA").then(|| "/l".to_owned());
        assert_eq!(slots_dir(&local), Some(PathBuf::from("/l/dx/slots")));
        let empty_home = |name: &str| match name {
            "HOME" => Some(String::new()),
            "LOCALAPPDATA" => Some("/l".to_owned()),
            _ => None,
        };
        assert_eq!(slots_dir(&empty_home), Some(PathBuf::from("/l/dx/slots")));
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
        let (sender, receiver) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let mut holders = Vec::new();
            let slot = acquire(&second_dir, 1, "tree-b", &mut |holder| {
                holders.push(holder.to_owned());
                let _ = sender.send(());
            })
            .expect("second");
            (slot.index, holders)
        });
        announced(&receiver);
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
        let queued = enqueue(&dir, "").expect("queued");
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
    fn a_leaving_ticket_never_deletes_the_next_waiters_ticket() {
        let scratch = dx_test_scratch::scratch("dx-slots-leave-");
        let dir = scratch.path().join("slots");
        let mut first = enqueue(&dir, "").expect("first");
        let mut next = None;
        first.leave(|| next = Some(enqueue(&dir, "").expect("next")));
        let next = next.expect("next ticket");
        drop(first);
        assert!(next.path.exists());
        let listed = tickets(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].1, next.path);
    }

    #[test]
    fn a_dropped_ticket_leaves_no_file() {
        let scratch = dx_test_scratch::scratch("dx-slots-drop-");
        let dir = scratch.path().join("slots");
        drop(enqueue(&dir, "").expect("ticket"));
        assert!(tickets(&dir).is_empty());
    }

    #[test]
    fn a_stale_ticket_does_not_block() {
        let scratch = dx_test_scratch::scratch("dx-slots-stale-");
        let dir = scratch.path().join("slots");
        let stale = enqueue(&dir, "").expect("stale");
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
        assert!(enqueue(&dir, "").is_err());
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
        assert_eq!(holder(&dir, 0..3, 2), "a and 2 more, 2 ahead in line");
        drop(a);
        assert!(!holder_path(&dir, 0).exists());
        assert_eq!(holder(&dir, 0..3, 0), "b and 1 more");
        drop((b, c));
        assert_eq!(holder(&dir, 0..3, 0), "another build");
    }

    #[test]
    fn a_free_slot_is_taken_without_waiting() {
        let scratch = dx_test_scratch::scratch("dx-slots-free-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 2, "a", &mut |_| {}).expect("first");
        let second = acquire(&dir, 2, "b", &mut |_| panic!("must not wait")).expect("second");
        assert_eq!((first.index, second.index), (0, 1));
    }

    /// A writer that buffers everything and signals once on its first write.
    ///
    /// `Gate::take` writes the wait line from `on_wait`, which `acquire_reserving` runs only after
    /// the waiter is queued and failed to take a slot, so the signal proves the waiter is in line.
    struct Announce {
        buf: Vec<u8>,
        signal: Option<std::sync::mpsc::Sender<()>>,
    }

    impl Announce {
        fn new() -> (Self, std::sync::mpsc::Receiver<()>) {
            let (signal, rx) = std::sync::mpsc::channel();
            let announce = Self {
                buf: Vec::new(),
                signal: Some(signal),
            };
            (announce, rx)
        }

        fn text(self) -> String {
            String::from_utf8(self.buf).expect("utf8")
        }
    }

    impl Write for Announce {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.buf.extend_from_slice(data);
            if let Some(signal) = self.signal.take() {
                let _ = signal.send(());
            }
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(30);

    /// Waits for the waiter's announcement; panics rather than hanging when it never comes.
    fn announced(rx: &std::sync::mpsc::Receiver<()>) {
        rx.recv_timeout(ANNOUNCE_TIMEOUT)
            .expect("waiter never announced a wait");
    }

    /// Waits for the announcement, then always drops `held`, so a missing wait fails the caller's
    /// assertions instead of hanging `hold()` forever. Returns whether the wait was announced.
    fn released_after_wait<T>(rx: &std::sync::mpsc::Receiver<()>, held: T) -> bool {
        let seen = rx.recv_timeout(ANNOUNCE_TIMEOUT).is_ok();
        drop(held);
        seen
    }

    /// Like `take_gate` on a thread; the receiver fires once the thread is waiting in line.
    fn spawn_take(
        gate: Gate,
    ) -> (
        std::thread::JoinHandle<(Slot, String, String)>,
        std::sync::mpsc::Receiver<()>,
    ) {
        let (mut err, rx) = Announce::new();
        let waiter = std::thread::spawn(move || {
            let mut events = Vec::new();
            let slot = gate
                .take(Command::Test, &mut err, &mut events)
                .expect("take")
                .expect("slot");
            (slot, err.text(), String::from_utf8(events).expect("utf8"))
        });
        (waiter, rx)
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
        assert!(slot_settles_free(&dir, 2));
    }

    fn count(n: usize) -> BuildSlots {
        BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"))
    }

    #[test]
    fn auto_sizing_matches_memory_sizing() {
        for (memory, reserved) in [
            (64u64 << 30, 0u64),
            (64 << 30, 6 << 30),
            (2 << 30, 0),
            (19 << 30, 6 << 30),
        ] {
            let count = slot_count(memory, reserved);
            assert_eq!(
                sizing(BuildSlots::Auto, memory, reserved),
                Some(Sizing {
                    count,
                    pool: slot_count(memory, 0),
                    extras: remote_slot_count(memory, reserved).saturating_sub(count),
                }),
                "{memory} {reserved}"
            );
        }
    }

    #[test]
    fn count_sizing_keeps_the_reserve_width_above_n() {
        let memory = 30u64 << 30;
        assert_eq!(
            sizing(count(2), memory, 0),
            Some(Sizing {
                count: 2,
                pool: 2,
                extras: 0
            })
        );
        let width = slot_count(memory, 0) - slot_count(memory, 6 << 30);
        assert_eq!(width, 2);
        assert_eq!(
            sizing(count(2), memory, 6 << 30),
            Some(Sizing {
                count: 2,
                pool: 2 + width,
                extras: 0
            })
        );
        assert_eq!(
            sizing(count(40), memory, 0),
            Some(Sizing {
                count: 40,
                pool: 40,
                extras: 0
            }),
            "an explicit count is not clamped to {MAX_SLOTS}"
        );
        assert_eq!(sizing(BuildSlots::Off, memory, 0), None);
    }

    #[test]
    fn off_and_nested_runs_build_no_gate() {
        let scratch = dx_test_scratch::scratch("dx-slots-off-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let get = |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        assert!(Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Off).is_none());
        let gate = Gate::from_env(Path::new("/ws"), &get, false, &[], count(3)).expect("gate");
        assert_eq!((gate.count, gate.pool, gate.extras), (3, 3, 0));
        let nested = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            SLOT_ENV_VAR => Some("1".to_owned()),
            _ => None,
        };
        for setting in [BuildSlots::Auto, BuildSlots::Off, count(1), count(40)] {
            assert!(
                Gate::from_env(Path::new("/ws"), &nested, false, &[], setting).is_none(),
                "a child under a held slot ignores {setting}"
            );
        }
    }

    #[test]
    fn a_count_gate_waits_on_the_take_past_n() {
        let scratch = dx_test_scratch::scratch("dx-slots-count-wait-");
        let dir = scratch.path().join("slots");
        let size = sizing(count(2), 64 << 30, 0).expect("sizing");
        let gate = Gate::new(dir.clone(), size.count, "fill".to_owned(), false)
            .reserving(size.pool)
            .extra(size.extras, false);
        let (first, _, _) = take_gate(&gate);
        let (second, _, _) = take_gate(&gate);
        assert_eq!((first.index, second.index), (0, 1));
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(first);
        let (third, err, _) = waiter.join().expect("join");
        assert_eq!(third.index, 0);
        assert!(
            err.starts_with("dx: waiting for a build slot (held by fill"),
            "{err}"
        );
        drop(second);
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
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(consumer);
        let (slot, err, events) = waiter.join().expect("join");
        assert_eq!(slot.index, 0);
        assert_eq!(err, "dx: waiting for a build slot (held by consumer)\n");
        assert_eq!(
            events.matches("waiting_for_build_slot").count(),
            1,
            "{events}"
        );
    }

    #[test]
    fn a_want_round_trips_and_garbage_wants_everything() {
        let want = Want {
            base: 0..3,
            reserve: 3..5,
            extras: 5..7,
            remote: true,
        };
        assert_eq!(Want::parse(&want.render()), Some(want));
        for garbage in [
            "",
            "base=0..1",
            "v=1 base=x",
            "v=2 base=0..1 reserve=1..1 extras=1..1 remote=0",
        ] {
            assert_eq!(Want::parse(garbage), None, "{garbage}");
        }
        let scratch = dx_test_scratch::scratch("dx-slots-want-garbage-");
        assert_eq!(
            Want::read(&scratch.path().join("missing.want")),
            Want::everything()
        );
    }

    #[test]
    fn a_dropped_ticket_removes_its_want_and_the_sweep_removes_orphans() {
        let scratch = dx_test_scratch::scratch("dx-slots-want-drop-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(queue_dir(&dir)).expect("queue");
        let want = Want::of(
            &Layout {
                count: 1,
                reserve: 1..2,
                extras: 2..2,
            },
            false,
        );
        let ticket = enqueue_wanting(&dir, "", &want).expect("ticket");
        let path = want_path(&ticket.path);
        assert_eq!(Want::read(&path), want);
        drop(ticket);
        assert!(!path.exists());
        let orphan = queue_dir(&dir).join("ticket-00000000000000000099.want");
        std::fs::write(&orphan, "x").expect("orphan");
        drop(enqueue(&dir, "").expect("ticket"));
        assert!(!orphan.exists());
    }

    #[test]
    fn a_consumer_passes_a_rules_dx_head_blocked_on_its_reserve() {
        let scratch = dx_test_scratch::scratch("dx-slots-pass-head-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
        let consumer = try_take(&dir, 2, "consumer")
            .expect("try")
            .expect("consumer");
        assert_eq!(consumer.index, 1);
        drop(fill);
        let (sender, receiver) = std::sync::mpsc::channel();
        let head_dir = dir.clone();
        let head = std::thread::spawn(move || {
            let layout = Layout {
                count: 1,
                reserve: 1..2,
                extras: 2..2,
            };
            acquire_reserving(
                &head_dir,
                &layout,
                Admission::none(),
                "r",
                &mut |holder, _| {
                    sender.send(holder.to_owned()).expect("send");
                },
                &mut || {},
                &mut |_, _| {},
            )
            .map(|slot| slot.index)
        });
        receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("head waits");
        let behind = acquire(&dir, 2, "behind", &mut |_| {}).expect("behind passes");
        assert_eq!(behind.index, 0);
        drop(behind);
        drop(consumer);
        assert_eq!(head.join().expect("join").expect("head"), 0);
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
            let layout = Layout {
                count: 1,
                reserve: 1..2,
                extras: 2..2,
            };
            acquire_reserving(
                &rules_dx_dir,
                &layout,
                Admission::none(),
                "r",
                &mut |holder, _| {
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
        drop(consumer);
        let held = rules_dx.join().expect("join");
        assert_eq!(held.index, 0);
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
        assert_eq!(notice, "r");
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
        let (mut err, rx) = Announce::new();
        let releaser = std::thread::spawn(move || released_after_wait(&rx, held));
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &get,
            &mut err,
            true,
            &mut events,
        );
        assert!(
            releaser.join().expect("join"),
            "waiter never announced a wait"
        );
        assert!(matches!(slot, Ok(Some(_))));
        let text = err.text();
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
        let (mut err, rx) = Announce::new();
        let releaser = std::thread::spawn(move || released_after_wait(&rx, held));
        let mut events = Vec::new();
        let slot = hold(
            command,
            Path::new("/ws"),
            &get,
            &mut err,
            false,
            &mut events,
        );
        assert!(
            releaser.join().expect("join"),
            "waiter never announced a wait"
        );
        assert!(matches!(slot, Ok(Some(_))), "{}", command.name());
        let text = err.text();
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
                &mut |_, _| {},
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

    type Reaps = Arc<Mutex<Vec<(Vec<PathBuf>, u64)>>>;

    fn recording(gate: Gate, stopped: usize) -> (Gate, Reaps) {
        let calls: Reaps = Arc::default();
        let seen = Arc::clone(&calls);
        let gate = gate.reaping(move |keep, target| {
            seen.lock().expect("reaps").push((keep.to_vec(), target));
            stopped
        });
        (gate, calls)
    }

    /// Makes `gate` wait behind a holder, then frees the slot; returns the wait text and JSON.
    fn wait_behind_holder(dir: &Path, gate: Gate) -> (String, String) {
        let holder = take_gate(&Gate::new(
            dir.to_path_buf(),
            1,
            "/w/holder".to_owned(),
            false,
        ))
        .0;
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(holder);
        let (_slot, err, events) = waiter.join().expect("join");
        (err, events)
    }

    fn wait_notice(events: &str) -> serde_json::Value {
        let notices: Vec<serde_json::Value> = events
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(notices.len(), 1, "{events}");
        assert_eq!(notices[0]["code"], "waiting_for_build_slot");
        notices[0].clone()
    }

    #[test]
    fn a_wait_reaps_once_keeping_holders_waiters_and_itself() {
        let scratch = dx_test_scratch::scratch("dx-slots-reap-");
        let dir = scratch.path().join("slots");
        let holder = take_gate(&Gate::new(dir.clone(), 1, "/w/holder".to_owned(), false)).0;
        let queued = enqueue(&dir, "/w/waiter").expect("queued");
        let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 2);
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        drop(queued);
        drop(holder);
        let (_slot, err, events) = waiter.join().expect("join");
        let calls = calls.lock().expect("reaps").clone();
        assert_eq!(calls.len(), 1);
        let (keep, target) = &calls[0];
        for workspace in ["/w/me", "/w/holder", "/w/waiter"] {
            assert!(keep.contains(&PathBuf::from(workspace)), "{keep:?}");
        }
        assert_eq!(*target, BYTES_PER_SLOT);
        assert_eq!(
            err.trim(),
            format!(
                "dx: {}; stopped 2 idle Bazel servers",
                wait_message("/w/holder, 1 ahead in line")
            )
        );
        let notice = wait_notice(&events);
        assert_eq!(notice["reaped_servers"], 2);
        assert!(notice["message"]
            .as_str()
            .is_some_and(|text| text.ends_with("; stopped 2 idle Bazel servers")));
    }

    #[test]
    fn a_wait_that_stops_nothing_keeps_the_plain_line_and_reports_zero() {
        let scratch = dx_test_scratch::scratch("dx-slots-reap-none-");
        let dir = scratch.path().join("slots");
        let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 0);
        let (err, events) = wait_behind_holder(&dir, gate);
        assert_eq!(calls.lock().expect("reaps").len(), 1);
        assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
        assert_eq!(wait_notice(&events)["reaped_servers"], 0);
    }

    #[test]
    fn a_gate_without_a_reaper_reports_no_reaped_servers() {
        let scratch = dx_test_scratch::scratch("dx-slots-no-reaper-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 1, "/w/me".to_owned(), true);
        let (err, events) = wait_behind_holder(&dir, gate);
        assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
        assert!(wait_notice(&events).get("reaped_servers").is_none());
    }

    #[test]
    fn another_reaping_waiter_skips_the_reap() {
        let scratch = dx_test_scratch::scratch("dx-slots-reap-busy-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(&dir).expect("dir");
        let busy = dx_atomic_fs::lease::try_lock_file(&dir.join("reap.lock"), true)
            .expect("lock")
            .expect("free");
        let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 3);
        let (err, events) = wait_behind_holder(&dir, gate);
        drop(busy);
        assert!(calls.lock().expect("reaps").is_empty());
        assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
        assert_eq!(wait_notice(&events)["reaped_servers"], 0);
    }

    #[test]
    fn a_free_slot_never_reaps() {
        let scratch = dx_test_scratch::scratch("dx-slots-reap-free-");
        let (gate, calls) = recording(
            Gate::new(scratch.path().join("slots"), 1, "/w/me".to_owned(), true),
            1,
        );
        let (_slot, err, events) = take_gate(&gate);
        assert!(calls.lock().expect("reaps").is_empty());
        assert!(err.is_empty() && events.is_empty());
    }

    #[test]
    fn a_remote_gate_reaps_one_remote_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-reap-remote-");
        let dir = scratch.path().join("slots");
        let (gate, calls) = recording(
            Gate::new(dir.clone(), 1, "/w/me".to_owned(), false).extra(0, true),
            0,
        );
        wait_behind_holder(&dir, gate);
        assert_eq!(calls.lock().expect("reaps")[0].1, BYTES_PER_REMOTE_SLOT);
    }

    #[test]
    fn reaped_suffix_counts_servers() {
        assert_eq!(reaped_suffix(0), "");
        assert_eq!(reaped_suffix(1), "; stopped 1 idle Bazel server");
        assert_eq!(reaped_suffix(3), "; stopped 3 idle Bazel servers");
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
            &mut |_, _| {},
            &mut || polls += 1,
            &mut |_, _| {},
        )
        .expect("slot");
        assert_eq!(slot.index, 0);
        assert_eq!(polls, 0);
    }

    #[test]
    fn ping_interval_is_a_third_of_the_idle_limit_within_bounds() {
        assert_eq!(ping_interval(Some(90)), Some(Duration::from_secs(30)));
        assert_eq!(ping_interval(Some(300)), Some(Duration::from_secs(60)));
        assert_eq!(ping_interval(Some(900)), Some(Duration::from_secs(60)));
        assert_eq!(ping_interval(None), Some(Duration::from_secs(60)));
        assert_eq!(ping_interval(Some(2)), Some(Duration::from_secs(1)));
        assert_eq!(ping_interval(Some(0)), None);
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
    fn a_server_that_never_idles_out_is_never_pinged() {
        let (mut keep, log) = keeper(Some((7, Some(0))));
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
