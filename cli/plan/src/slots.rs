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
    BYTES_PER_NESTED_BAZEL, BYTES_PER_SLOT, MAX_SLOTS, available_memory, cgroup_limit_in,
    lock_slot, parse_cgroup_limit, parse_meminfo, slot_count, slot_path, slots_dir,
};
use dx_worktree::host::{self, Host, Server, SystemHost};

/// Notice code for the one info notice a run prints while it waits for a free build slot.
pub const WAITING_FOR_BUILD_SLOT_CODE: &str = "waiting_for_build_slot";
/// Notice code for a build slot the run could not take and skipped.
pub const BUILD_SLOT_SKIPPED_CODE: &str = "build_slot_skipped";
/// Failure code for a run that waited past its limit without getting a build slot.
pub const SLOTS_BUSY_CODE: &str = "slots_busy";
/// Notice code for a run that turned build slots off after an error.
pub const BUILD_SLOTS_DISABLED_CODE: &str = "build_slots_disabled";
/// Notice code for the disk reclaim a build slot runs before a build.
pub const DISK_LOW_CODE: &str = "disk_low";

/// Seconds a `dx worktree` slot's Bazel server may sit idle before it exits.
pub const SLOT_MAX_IDLE_SECS: u64 = 300;

/// The `--max_idle_secs` for the server of `workspace`: short inside a `dx worktree` slot, `None` elsewhere.
pub fn slot_idle_secs(workspace: &Path) -> Option<u64> {
    dx_worktree::lease_for_dir(workspace).map(|_| SLOT_MAX_IDLE_SECS)
}

/// Sets the memory one build slot is planned to use: MiB, or a count with an `m` or `g` suffix.
pub const SLOT_MEMORY_ENV: &str = "DX_BUILD_SLOT_MEMORY";

/// Reads a slot memory size in bytes; `None` for zero or anything unreadable.
pub fn parse_slot_memory(value: &str) -> Option<u64> {
    let value = value.trim().to_ascii_lowercase();
    let (digits, unit) = match value.chars().last()? {
        'g' => (&value[..value.len() - 1], 30),
        'm' => (&value[..value.len() - 1], 20),
        _ => (value.as_str(), 20),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let bytes = digits.parse::<u64>().ok()?.checked_mul(1 << unit)?;
    (bytes > 0).then_some(bytes)
}

/// Memory one local build slot is planned to use: `DX_BUILD_SLOT_MEMORY` when set and valid.
pub fn slot_bytes(env_get: &dyn Fn(&str) -> Option<String>) -> u64 {
    env_get(SLOT_MEMORY_ENV)
        .and_then(|value| parse_slot_memory(&value))
        .unwrap_or(BYTES_PER_SLOT)
}

/// `bytes` rescaled so that dividing by the default slot size counts slots of `per_slot` bytes.
fn per_slot_scaled(bytes: u64, per_slot: u64) -> u64 {
    let scaled = u128::from(bytes) * u128::from(BYTES_PER_SLOT) / u128::from(per_slot);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

pub mod pressure;

use pressure::{HostPressure, NoSignal, PRESSURE_WAIT, Pressure, PressurePause};

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
        self.inner
            .run_app(argv, cwd, &slot_env(env, share.as_deref()), withdraw, held)
    }

    fn pinned_tool(&self, var: &str) -> dx_process::PinnedVar {
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
            | Command::Bench
            | Command::Lint
            | Command::Typecheck
            | Command::Format
            | Command::Check
            | Command::Deploy
            | Command::Generate
            | Command::Sync
            | Command::Security
            | Command::License
            | Command::Docs
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
    let read = dx_process::rc::read_file;
    let read_user_rc = dx_process::RcPolicy::from_env(env_get, workspace, &read).read_user_rc;
    if !dx_process::rc::nested_bazel_selected(env_get, workspace, &read, read_user_rc, &[], &[]) {
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

impl Sizing {
    /// Every slot index a held count looks at: the base slots, then the remote extras.
    fn counted(&self) -> impl Iterator<Item = usize> {
        (0..self.count).chain(self.pool..self.pool + self.extras)
    }
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

/// The build slot setting to use: the one given, else `Auto` inside a `dx worktree` slot and `Off` elsewhere.
pub fn effective_setting(set: Option<BuildSlots>, workspace: &Path) -> BuildSlots {
    set.unwrap_or_else(|| {
        if dx_worktree::lease_for_dir(workspace).is_some() {
            BuildSlots::Auto
        } else {
            BuildSlots::Off
        }
    })
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
    let per_slot = slot_bytes(env_get);
    let size = sizing(
        setting,
        per_slot_scaled(memory, per_slot),
        per_slot_scaled(reserved, per_slot),
    )?;
    let cpus = std::thread::available_parallelism().map_or(1, |cpus| cpus.get());
    let cpus = u64::try_from(cpus).unwrap_or(1);
    // `usize::MAX` is no slot, so `held` counts every held slot plus the new run.
    let with_new = held(&dir, size.counted(), usize::MAX);
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
        .filter(|&index| index != own && slot_path(dir, index).exists())
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
        .filter_map(|(_, path)| read(&label_path(&path)).or_else(|| read(&path)));
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
    /// Removes the want file, runs `between`, then removes the ticket path and
    /// releases the lock. Does nothing once the file is gone, so a later drop
    /// cannot remove a path a newer waiter has reused.
    fn leave(&mut self, between: impl FnOnce()) {
        if let Some(file) = self.file.take() {
            let _ = std::fs::remove_file(want_path(&self.path));
            let _ = std::fs::remove_file(label_path(&self.path));
            between();
            let _ = std::fs::remove_file(&self.path);
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
            "base={} reserve={} extras={} remote={}\n",
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
        let (mut base, mut reserve, mut extras, mut remote) = (None, None, None, None);
        for field in text.split_whitespace() {
            let (key, value) = field.split_once('=')?;
            match key {
                "base" => base = range(value),
                "reserve" => reserve = range(value),
                "extras" => extras = range(value),
                "remote" => remote = Some(value == "1"),
                _ => return None,
            }
        }
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

/// The waiter's label, readable while the ticket itself is locked.
fn label_path(ticket: &Path) -> PathBuf {
    ticket.with_extension("label")
}

fn is_forbidden(forbidden: &[Range<usize>], index: usize) -> bool {
    forbidden.iter().any(|range| range.contains(&index))
}

/// How long ago `time` was; a time in the future counts as infinitely old.
fn age(time: SystemTime) -> Duration {
    time.elapsed().unwrap_or(Duration::MAX)
}

fn waiting(path: &Path) -> bool {
    let fresh = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|time| age(time) < STALE);
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
    let _ = std::fs::write(label_path(&path), label);
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
        if path
            .extension()
            .is_some_and(|ext| ext == "want" || ext == "label")
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
        .is_ok_and(|time| age(time) < EXTRA_SPACING)
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
    /// When to stop waiting; `None` waits for ever.
    deadline: Option<Instant>,
}

impl Admission<'static> {
    /// No pressure source: never held back, never takes an extra.
    fn none() -> Self {
        Admission {
            pressure: None,
            remote: false,
            pause: PressurePause::new(PRESSURE_WAIT),
            now: &Instant::now,
            deadline: None,
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
    let ticket = match enqueue_wanting(dir, label, &Want::of(layout, admission.remote)) {
        Ok(ticket) => Some(ticket),
        Err(error) => {
            // A slot directory that cannot be opened at all is reported by the caller.
            if dir.is_dir() {
                on_skip(&queue_dir(dir), &error);
            }
            None
        }
    };
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
        if admission
            .deadline
            .is_some_and(|limit| (admission.now)() >= limit)
        {
            let names = holder(dir, layout.named(), ahead);
            return Err(io::Error::new(io::ErrorKind::TimedOut, Busy(names)));
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

/// Carried by the `TimedOut` error of a wait that reached its deadline: who held the slots.
#[derive(Debug)]
struct Busy(String);

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Busy {}

/// Why a slot was not taken: a stop signal, or no slot freed within the wait limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    Signal(i32),
    /// No slot freed in time; carries who held them.
    Busy(String),
}

fn stopped(signo: i32) -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        format!("stopped by signal {signo}"),
    )
}

/// The one line printed while waiting.
pub fn wait_message(holder: &str) -> String {
    format!(
        "waiting for a build slot (held by {holder}); see dx status, or set --build-slots or DX_BUILD_SLOTS"
    )
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
    wait: Option<Duration>,
    slot_bytes: u64,
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
            wait: None,
            slot_bytes: BYTES_PER_SLOT,
        }
    }

    /// Plans each local slot to use `bytes` of memory.
    pub fn slot_memory(self, bytes: u64) -> Self {
        Gate {
            slot_bytes: bytes,
            ..self
        }
    }

    /// Gives up on a slot after waiting this long; zero fails when none is free at once.
    pub fn waiting_at_most(self, wait: Option<Duration>) -> Self {
        Gate { wait, ..self }
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
            self.slot_bytes
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
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        Self::from_args(workspace, env_get, json, bazel_options, setting, &args)
    }

    /// `from_env` with an explicit argv, which excludes argv[0].
    fn from_args(
        workspace: &Path,
        env_get: &dyn Fn(&str) -> Option<String>,
        json: bool,
        bazel_options: &[String],
        setting: BuildSlots,
        args: &[OsString],
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
        let per_slot = slot_bytes(env_get);
        let Sizing {
            count,
            pool,
            extras,
        } = sizing(
            setting,
            per_slot_scaled(memory, per_slot),
            per_slot_scaled(reserved, per_slot),
        )?;
        let read = dx_process::rc::read_file;
        let read_user_rc = dx_process::RcPolicy::from_env(env_get, workspace, &read).read_user_rc;
        let startup = own_startup_options(args);
        let remote = dx_process::rc::remote_execution(
            env_get,
            workspace,
            &read,
            read_user_rc,
            startup.as_deref().unwrap_or_default(),
            bazel_options,
        );
        let cpus = std::thread::available_parallelism().map_or(1, |cpus| cpus.get());
        let cpus = u64::try_from(cpus).unwrap_or(1);
        let paths = dx_process::rc::startup_output_paths(
            env_get,
            workspace,
            &read,
            read_user_rc,
            startup.as_deref().unwrap_or_default(),
        );
        let default_roots = host::default_output_roots(env_get);
        let roots = reap_roots(&default_roots, &paths);
        let warm = startup.map(|startup_options| Warmth {
            workspace: workspace.to_path_buf(),
            startup_options,
            output_user_root: paths.output_user_root.clone(),
            output_base: paths.output_base.clone(),
            default_roots,
        });
        let reclaim_in = workspace.to_path_buf();
        Some(Gate {
            warm,
            ..Gate::new(dir, count, workspace.display().to_string(), json)
                .reserving(pool)
                .slot_memory(per_slot)
                .extra(extras, remote)
                .with_pressure(Arc::new(HostPressure::new(if remote {
                    BYTES_PER_REMOTE_SLOT
                } else {
                    per_slot
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
    ) -> Result<Option<Slot>, Stop> {
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
    ) -> Result<Option<Slot>, Stop> {
        self.take_warming(command, err, events, self.warm.clone())
    }

    /// `take` with an explicit keep-warm source, ticked while waiting for a slot.
    fn take_warming<W: Warm>(
        &self,
        command: Command,
        err: &mut dyn Write,
        events: &mut dyn Write,
        warm: Option<W>,
    ) -> Result<Option<Slot>, Stop> {
        let json = self.json;
        let mut keep = warm.map(KeepWarm::new);
        let out = RefCell::new((err, events));
        let layout = self.layout();
        let admission = Admission {
            pressure: Some(&*self.pressure),
            remote: self.remote,
            pause: PressurePause::new(PRESSURE_WAIT),
            now: &*self.clock.0,
            deadline: self
                .wait
                .and_then(|wait| (self.clock.0)().checked_add(wait)),
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
                    return Err(Stop::Signal(signo));
                }
                if let Some(Busy(holder)) = error.get_ref().and_then(|inner| inner.downcast_ref()) {
                    return Err(Stop::Busy(holder.clone()));
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
    /// The `--output_user_root` and `--output_base` in effect (rc files and command line).
    output_user_root: Option<PathBuf>,
    output_base: Option<PathBuf>,
    default_roots: Vec<PathBuf>,
}

/// The output roots the reaper acts on: the defaults plus the root rc files or the command line set.
fn reap_roots(defaults: &[PathBuf], paths: &dx_process::rc::StartupPaths) -> Vec<PathBuf> {
    let mut roots = defaults.to_vec();
    roots.extend(paths.output_user_root.clone());
    roots
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

fn default_output_base(dir: &Path, root: &Path) -> bool {
    dir.parent()
        .is_some_and(|parent| host::same_path(parent, root))
        && dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.len() == 32 && name.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The one running server a Bazel call from `workspace` with this `root` and `base` would reach.
fn own_server<'a>(
    servers: &'a [Server],
    workspace: &Path,
    root: Option<&Path>,
    base: Option<&Path>,
    default_roots: &[PathBuf],
) -> Option<&'a Server> {
    let mut found = servers.iter().filter(|server| {
        let (Some(dir), Some(output_base), Some(user_root)) = (
            server.workspace.as_deref(),
            server.output_base.as_deref(),
            server.output_user_root.as_deref(),
        ) else {
            return false;
        };
        let root_matches = match root {
            Some(root) => host::same_path(user_root, root),
            None => default_roots
                .iter()
                .any(|default| host::same_path(user_root, default)),
        };
        let base_matches = match base {
            Some(base) => host::same_path(output_base, base),
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

impl Warmth {
    /// The full command a ping runs: launcher, startup options, then a lock-free `info`.
    #[allow(clippy::disallowed_methods)]
    fn ping_argv(&self) -> Vec<String> {
        let mut argv = dx_process::startup_argv(&self.startup_options);
        argv.extend(["--noblock_for_lock", "info", "server_pid"].map(str::to_owned));
        argv
    }
}

impl Warm for Warmth {
    type Ping = Child;

    fn own_server(&self) -> Option<(u32, Option<u64>)> {
        let servers = SystemHost.servers();
        let server = own_server(
            &servers,
            &self.workspace,
            self.output_user_root.as_deref(),
            self.output_base.as_deref(),
            &self.default_roots,
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
        let argv = self.ping_argv();
        let (program, args) = argv.split_first()?;
        std::process::Command::new(program)
            .args(args)
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
        if let Warming::On { ping, .. } = &mut self.state
            && let Some(child) = ping.take()
        {
            self.warm.stop(child);
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
) -> Result<Option<Slot>, Stop> {
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
    ) -> Result<Option<&Slot>, Stop> {
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
const SETTLE_LIMIT: Duration = Duration::from_secs(5);
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
#[path = "slots_tests.rs"]
mod tests;
