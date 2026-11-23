use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::WorktreeError;
use super::cache;
use super::git;
use super::host::{
    DiskSpace, Download, Host, OutputBase, Server, download_used, same_path, scan_downloads,
    scan_output_bases,
};
use super::pool::{Pool, Record, Who};
use super::primary;

const STALE_AFTER_SECS: u64 = 30 * 24 * 60 * 60;
const EXTERNAL_GRACE_SECS: u64 = 24 * 60 * 60;
const FREE_LOCK_REASON: &str = "dx: free";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Held,
    Free,
    Reclaimable,
    External,
}

impl State {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            State::Held => "held",
            State::Free => "free",
            State::Reclaimable => "reclaimable",
            State::External => "external",
        }
    }
}

enum Start {
    Create(String),
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    pub branch: String,
    pub create: Option<String>,
    pub force: bool,
    pub unclaimed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub slot: u32,
    pub path: PathBuf,
    pub reused: bool,
    pub base: Option<String>,
    pub warnings: Vec<Warning>,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reclaim {
    pub(crate) warning: Warning,
    pub(crate) removed: bool,
    pub(crate) skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    pub slot: Option<u32>,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub already_free: bool,
    pub removed: bool,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub slot: Option<u32>,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub holder: Option<String>,
    pub state: State,
    pub reason: Option<&'static str>,
    pub server: bool,
    pub busy: bool,
    pub disk_bytes: Option<u64>,
    pub prunable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub rows: Vec<Row>,
    pub servers: u64,
    pub servers_rss_bytes: u64,
    pub disk_bytes: Option<u64>,
    pub disk_cache_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneSlot {
    pub slot: u32,
    pub path: PathBuf,
    pub idle_days: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseOwner {
    Orphan,
    Slot(u32),
}

impl BaseOwner {
    #[must_use]
    pub fn slot(&self) -> Option<u32> {
        match self {
            BaseOwner::Slot(slot) => Some(*slot),
            BaseOwner::Orphan => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneBase {
    pub dir: PathBuf,
    pub workspace: PathBuf,
    pub bytes: u64,
    pub owner: BaseOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneCache {
    pub slot: u32,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneSkippedCache {
    pub slot: u32,
    pub path: PathBuf,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneKeptBase {
    pub dir: PathBuf,
    pub workspace: PathBuf,
    pub owner: BaseOwner,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneExternal {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneKept {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneDownloads {
    pub cache: PathBuf,
    pub entries: Vec<Download>,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneSkippedSlot {
    pub slot: u32,
    pub path: PathBuf,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Applied {
    pub(crate) kept: Vec<PruneKept>,
    pub(crate) skipped_slots: Vec<PruneSkippedSlot>,
    pub(crate) skipped_bases: Vec<PruneKeptBase>,
    pub(crate) skipped_caches: Vec<PruneSkippedCache>,
    pub(crate) used_downloads: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pruned {
    pub plan: PrunePlan,
    pub kept: Vec<PruneKept>,
    pub skipped_slots: Vec<PruneSkippedSlot>,
    pub skipped_bases: Vec<PruneKeptBase>,
    pub skipped_caches: Vec<PruneSkippedCache>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrunePlan {
    pub slots: Vec<PruneSlot>,
    pub externals: Vec<PruneExternal>,
    pub bases: Vec<PruneBase>,
    pub caches: Vec<PruneCache>,
    pub kept_bases: Vec<PruneKeptBase>,
    pub kept_externals: Vec<PruneKept>,
    pub downloads: Vec<PruneDownloads>,
}

impl PrunePlan {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
            && self.externals.is_empty()
            && self.bases.is_empty()
            && self.caches.is_empty()
            && self.downloads.is_empty()
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.slots.iter().map(|slot| slot.bytes).sum::<u64>()
            + self
                .externals
                .iter()
                .map(|external| external.bytes)
                .sum::<u64>()
            + self.bases.iter().map(|base| base.bytes).sum::<u64>()
            + self.caches.iter().map(|cache| cache.bytes).sum::<u64>()
            + self
                .downloads
                .iter()
                .map(|downloads| downloads.bytes)
                .sum::<u64>()
    }
}

pub const STOP_GRACE_SECS: u64 = 30 * 60;

pub(crate) fn holder_alive(host: &dyn Host, record: &Record) -> bool {
    holder_running(host, record) || holder_recent(host, record)
}

pub(crate) fn holder_running(host: &dyn Host, record: &Record) -> bool {
    let Some(pid) = record.pid.filter(|pid| host.process_alive(*pid)) else {
        return false;
    };
    let differs = |recorded: &Option<String>, current: Option<String>| matches!((recorded, current), (Some(old), Some(new)) if *old != new);
    !record.gone
        && !differs(&record.boot_id, host.boot_id())
        && !differs(&record.pid_start, host.process_start(pid))
}

fn stamp(host: &dyn Host, record: &mut Record) {
    record.pid_start = record.pid.and_then(|pid| host.process_start(pid));
    record.boot_id = host.boot_id();
}

fn holder_recent(host: &dyn Host, record: &Record) -> bool {
    let last = record.stopped_at.unwrap_or(0).max(record.last_used);
    host.now().saturating_sub(last) < STOP_GRACE_SECS
}

enum Hold {
    Free,
    Alive,
    Reclaimable,
    Waiting(&'static str),
}

fn hold(pool: &Pool, host: &dyn Host, record: &Record) -> Hold {
    hold_known(pool, host, record, record.held && pool.busy(record.slot))
}

fn hold_known(pool: &Pool, host: &dyn Host, record: &Record, busy: bool) -> Hold {
    if !record.held {
        return Hold::Free;
    }
    let path = pool.slot_path(record.slot);
    if busy || holder_alive(host, record) || host.process_in(&path) {
        return Hold::Alive;
    }
    if !matches!(git::dirty(&path, &|| host.interrupted()), Ok(false)) {
        return Hold::Waiting("holder gone, tree has uncommitted changes");
    }
    if !matches!(git::unpushed(&path, &|| host.interrupted()), Ok(0)) {
        return Hold::Waiting("holder gone, commits are not pushed");
    }
    Hold::Reclaimable
}

struct SurplusLimit {
    keep: u32,
    total: usize,
    low_disk: bool,
}

const SCAN_EXTRA: usize = 4;

fn surplus_slots(
    pool: &Pool,
    host: &dyn Host,
    records: &[Record],
    limit: &SurplusLimit,
) -> Vec<u32> {
    let max = host.max_slots();
    if max == 0 {
        return Vec::new();
    }
    let mut want = limit.total.saturating_sub(max);
    if want == 0 && limit.low_disk {
        want = 1;
    }
    if want == 0 {
        return Vec::new();
    }
    let mut candidates: Vec<&Record> = records
        .iter()
        .filter(|record| record.slot != limit.keep && !record.held && !record.broken)
        .collect();
    candidates.sort_by_key(|record| (record.last_used, record.slot));
    let mut picked = Vec::new();
    for record in candidates.into_iter().take(want + SCAN_EXTRA) {
        if picked.len() == want || host.interrupted().is_some() {
            break;
        }
        if deletable(pool, host, record) {
            picked.push(record.slot);
        }
    }
    picked
}

fn deletable(pool: &Pool, host: &dyn Host, record: &Record) -> bool {
    if record.held || record.broken {
        return false;
    }
    let path = pool.slot_path(record.slot);
    if pool.busy(record.slot) || host.process_in(&path) {
        return false;
    }
    let stop = || host.interrupted();
    matches!(git::operation_in_progress(&path, &stop), Ok(false))
        && matches!(git::holds_files(&path, &stop), Ok(false))
        && matches!(git::unpushed(&path, &stop), Ok(0))
}

fn delete_surplus(pool: &Pool, host: &dyn Host, keep: u32, low_disk: bool) {
    let records = pool.records();
    let total = records.iter().filter(|record| !record.broken).count();
    let picked = surplus_slots(
        pool,
        host,
        &records,
        &SurplusLimit {
            keep,
            total,
            low_disk,
        },
    );
    if picked.is_empty() {
        return;
    }
    let Ok(_lock) = pool.lock() else {
        return;
    };
    let bases = scan_output_bases(&host.output_roots(&host.servers()));
    for slot in picked {
        let Some(record) = pool.record(slot) else {
            continue;
        };
        if slot != keep && deletable(pool, host, &record) {
            drop(delete_slot(pool, host, slot, &bases));
        }
    }
}

fn reclaimable(pool: &Pool, host: &dyn Host, record: &Record) -> bool {
    matches!(hold(pool, host, record), Hold::Reclaimable)
}

fn describe_holder(pool: &Pool, host: &dyn Host, record: &Record) -> String {
    record
        .holder(pool.busy(record.slot) || holder_alive(host, record))
        .unwrap_or_else(|| "nobody".to_owned())
}

fn resolve_start(pool: &Pool, branch: &str, create: Option<&str>) -> Result<Start, WorktreeError> {
    let main = &pool.main;
    if let Some(requested) = create {
        if git::branch_exists(main, branch) {
            return Err(WorktreeError::usage(
                "branch_exists",
                &format!("branch {branch} already exists; run the command without --create"),
            ));
        }
        if !git::ref_exists(main, requested) {
            return Err(WorktreeError::usage(
                "unknown_ref",
                &format!(
                    "--create base {requested} does not name a commit; pass --create=origin/<branch> or a commit"
                ),
            ));
        }
        return Ok(Start::Create(requested.to_owned()));
    }
    if git::branch_exists(main, branch) {
        return Ok(Start::Local);
    }
    if git::ref_exists(main, &format!("origin/{branch}")) {
        return Ok(Start::Remote);
    }
    Ok(Start::Create(git::default_base(main)?))
}

fn target_ref(start: &Start, branch: &str) -> String {
    match start {
        Start::Create(base) => base.clone(),
        Start::Local => branch.to_owned(),
        Start::Remote => format!("origin/{branch}"),
    }
}

fn rank(candidates: &mut Vec<&Record>, branch: &str, digest: Option<&str>) {
    candidates.sort_by_key(|record| {
        (
            record.last_branch.as_deref() != Some(branch),
            digest.is_none() || record.lock_digest.as_deref() != digest,
            std::cmp::Reverse(record.last_used),
            record.slot,
        )
    });
}

fn busy_error(pool: &Pool, slot: u32) -> WorktreeError {
    WorktreeError::refused(
        "slot_busy",
        &format!(
            "slot {slot} at {} is running a dx command; wait for it to finish",
            pool.slot_path(slot).display()
        ),
    )
}

fn first_quiet<'a>(
    pool: &Pool,
    ranked: &[&'a Record],
    quiet: &mut Option<std::fs::File>,
) -> Result<Option<&'a Record>, WorktreeError> {
    for record in ranked {
        if let Some(guard) = pool.try_quiet(record.slot)? {
            *quiet = Some(guard);
            return Ok(Some(record));
        }
    }
    Ok(None)
}

pub(crate) fn held_on<'a>(records: &'a [Record], branch: &str) -> Option<&'a Record> {
    records
        .iter()
        .find(|record| record.held && record.branch.as_deref() == Some(branch))
}

fn renew(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    record: &Record,
    now: u64,
) -> Result<Option<Warning>, WorktreeError> {
    if holder_alive(host, record) || who.pid == 0 || !host.process_alive(who.pid) {
        return Ok(None);
    }
    let mut renewed = record.clone();
    renewed.pid = Some(who.pid);
    stamp(host, &mut renewed);
    renewed.gone = false;
    renewed.last_used = now;
    pool.write(&renewed)?;
    Ok(Some(Warning {
        code: "holder_renewed",
        message: format!(
            "the recorded holder of slot {} was not running; it is now pid {}",
            record.slot, who.pid
        ),
    }))
}

fn reuse_held(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    branch: &str,
) -> Result<Option<Added>, WorktreeError> {
    let _lock = pool.lock()?;
    pool.forget_missing();
    let records = pool.records();
    let Some(record) = held_on(&records, branch).filter(|record| record.held_by(who)) else {
        return Ok(None);
    };
    let warnings = renew(pool, host, who, record, host.now())?
        .into_iter()
        .collect();
    Ok(Some(Added {
        slot: record.slot,
        path: pool.slot_path(record.slot),
        reused: true,
        base: None,
        warnings,
        notice: None,
    }))
}

pub fn add(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
) -> Result<Added, WorktreeError> {
    check_branch(pool, &request.branch)?;
    if let Some(added) = reuse_held(pool, host, who, &request.branch)? {
        return Ok(added);
    }
    let warnings = refresh(pool, host)?;
    lease(pool, host, who, request, &[], warnings)
}

pub(crate) fn check_branch(pool: &Pool, branch: &str) -> Result<(), WorktreeError> {
    if branch.is_empty() || !git::succeeds(&pool.main, &["check-ref-format", "--branch", branch]) {
        return Err(WorktreeError::usage(
            "invalid_branch",
            &format!(
                "{branch:?} is not a branch name; pick a name that git check-ref-format --branch accepts"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn refresh(pool: &Pool, host: &dyn Host) -> Result<Vec<Warning>, WorktreeError> {
    let mut warnings = Vec::new();
    stop_check(host)?;
    if git::succeeds(&pool.main, &["remote", "get-url", "origin"]) {
        let fetched = git::run_until(
            &pool.main,
            &["fetch", "--quiet", "origin"],
            Duration::from_secs(300),
            &|| host.interrupted(),
        );
        if let Err(error) = fetched {
            stop_check(host)?;
            warnings.push(Warning {
                code: "fetch_failed",
                message: format!("cannot fetch origin, using local refs: {error}"),
            });
        }
    }
    Ok(warnings)
}

pub(crate) fn lease(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
    skip: &[u32],
    warnings: Vec<Warning>,
) -> Result<Added, WorktreeError> {
    let lock = pool.lock()?;
    lease_locked(pool, host, who, request, skip, warnings, lock)
}

pub(crate) fn lease_locked(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
    skip: &[u32],
    mut warnings: Vec<Warning>,
    mut lock: dx_atomic_fs::lease::ExclusiveLease,
) -> Result<Added, WorktreeError> {
    let mut skip = skip.to_vec();
    loop {
        match lease_once(pool, host, who, request, &skip, warnings, lock) {
            Ok(mut added) => {
                match primary::protect(&pool.main) {
                    Ok(notice) => added.notice = notice,
                    Err(error) => added.warnings.push(Warning {
                        code: "protect_failed",
                        message: format!(
                            "cannot set {} in the main checkout: {}",
                            primary::KEY,
                            error.message
                        ),
                    }),
                }
                match primary::guard_git(&pool.main) {
                    Ok(None) => {}
                    Ok(Some(message)) => added.warnings.push(Warning {
                        code: "git_hook_skipped",
                        message,
                    }),
                    Err(error) => added.warnings.push(Warning {
                        code: "git_hook_failed",
                        message: format!(
                            "cannot write the git hook of the main checkout: {}",
                            error.message
                        ),
                    }),
                }
                return Ok(added);
            }
            Err(Missed::Failed(error)) => return Err(error),
            Err(Missed::Unclean {
                slot,
                error,
                warnings: kept,
            }) => {
                warnings = kept;
                warnings.push(Warning {
                    code: "slot_skipped",
                    message: format!(
                        "skipped free slot {slot}: {}; fix its permissions or run `dx worktree prune --apply`",
                        error.message
                    ),
                });
                skip.push(slot);
                lock = pool.lock()?;
            }
        }
    }
}

enum Missed {
    Failed(WorktreeError),
    Unclean {
        slot: u32,
        error: WorktreeError,
        warnings: Vec<Warning>,
    },
}

impl From<WorktreeError> for Missed {
    fn from(error: WorktreeError) -> Self {
        Missed::Failed(error)
    }
}

fn lease_once(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
    skip: &[u32],
    mut warnings: Vec<Warning>,
    lock: dx_atomic_fs::lease::ExclusiveLease,
) -> Result<Added, Missed> {
    let branch = request.branch.as_str();
    pool.forget_missing();
    let now = host.now();
    let records = pool.records();
    let mut takeover: Option<&Record> = None;
    if let Some(record) = held_on(&records, branch) {
        let path = pool.slot_path(record.slot);
        if record.held_by(who) {
            warnings.extend(renew(pool, host, who, record, now)?);
            return Ok(Added {
                slot: record.slot,
                path,
                reused: true,
                base: None,
                warnings,
                notice: None,
            });
        }
        if request.force {
            if pool.busy(record.slot) {
                return Err(busy_error(pool, record.slot).into());
            }
            let mut adopted = record.clone();
            adopted.lease_to(who);
            stamp(host, &mut adopted);
            adopted.last_used = now;
            pool.write(&adopted)?;
            return Ok(Added {
                slot: record.slot,
                path,
                reused: true,
                base: None,
                warnings,
                notice: None,
            });
        }
        let held = hold(pool, host, record);
        stop_check(host)?;
        if !matches!(held, Hold::Reclaimable) {
            let mut message = format!(
                "branch {branch} is held by slot {} at {}, leased to {}",
                record.slot,
                path.display(),
                describe_holder(pool, host, record)
            );
            if let Hold::Waiting(reason) = held {
                message.push_str(&format!("; {reason}"));
            }
            message.push_str("; pick another branch or pass --force to take it over");
            return Err(WorktreeError::refused("branch_held", &message).into());
        }
        takeover = Some(record);
    }
    let mut quiet: Option<std::fs::File> = None;
    if let Some(record) = takeover {
        match pool.try_quiet(record.slot)? {
            Some(guard) => quiet = Some(guard),
            None => return Err(busy_error(pool, record.slot).into()),
        }
    }
    for checkout in pool.checkouts()? {
        if checkout.branch.as_deref() == Some(branch)
            && !takeover
                .is_some_and(|record| same_path(&checkout.path, &pool.slot_path(record.slot)))
        {
            return Err(WorktreeError::refused(
                "branch_held",
                &format!(
                    "branch {branch} is checked out at {p}; cd {p} or pick another branch",
                    p = checkout.path.display()
                ),
            )
            .into());
        }
    }
    let start = resolve_start(pool, branch, request.create.as_deref())?;
    let target = target_ref(&start, branch);
    let digest = git::lock_digest(&pool.main, &target);
    let mut free: Vec<&Record> = records
        .iter()
        .filter(|record| !record.held && !record.broken && !skip.contains(&record.slot))
        .collect();
    rank(&mut free, branch, digest.as_deref());
    let mut reclaimed = takeover.is_some();
    let mut chosen: Option<&Record> = takeover;
    if chosen.is_none() {
        chosen = first_quiet(pool, &free, &mut quiet)?;
    }
    if chosen.is_none() {
        let mut candidates: Vec<&Record> = records
            .iter()
            .filter(|record| !skip.contains(&record.slot) && reclaimable(pool, host, record))
            .collect();
        stop_check(host)?;
        rank(&mut candidates, branch, digest.as_deref());
        chosen = first_quiet(pool, &candidates, &mut quiet)?;
        reclaimed = chosen.is_some();
    }
    let stale: Vec<u32> = records
        .iter()
        .filter(|record| {
            !record.held
                && Some(record.slot) != chosen.map(|picked| picked.slot)
                && (record.broken || now.saturating_sub(record.last_used) >= STALE_AFTER_SECS)
        })
        .map(|record| record.slot)
        .collect();
    let slot = chosen.map_or_else(|| pool.next_slot(&records), |record| record.slot);
    let fresh = chosen.is_none();
    if fresh {
        quiet = pool.try_quiet(slot)?;
    }
    let _quiet = quiet;
    let low = low_disk(host, &pool.base);
    let low_now = low.is_some();
    let mut reclaim_warning =
        low.map(|low| reclaim_locked(pool, host, Some(slot), low, &lock).warning);
    let reuse = chosen
        .filter(|record| !record.held)
        .map(|record| record.slot);
    if let Some(dropped) = make_room(pool, host, branch, reuse)? {
        match &mut reclaim_warning {
            Some(warning) => {
                warning.message.push_str(&format!("; {dropped}"));
            }
            None => {
                reclaim_warning = Some(Warning {
                    code: "disk_low",
                    message: dropped,
                });
            }
        }
    }
    warnings.extend(reclaim_warning);
    if !host.process_alive(who.pid) {
        warnings.push(Warning {
            code: "holder_gone",
            message: format!(
                "holder pid {} is not running, so the slot can be reclaimed once its tree is clean and pushed; run add from the long-lived agent shell",
                who.pid
            ),
        });
    }
    let mut placeholder = Record {
        slot,
        held: true,
        branch: Some(branch.to_owned()),
        last_branch: chosen.and_then(|record| record.branch.clone().or(record.last_branch.clone())),
        session: who.session.clone(),
        agent: who.agent.clone(),
        pid: Some(who.pid),
        pid_start: None,
        boot_id: None,
        gone: false,
        stopped_at: None,
        unclaimed: request.unclaimed,
        broken: false,
        leased_at: now,
        last_used: now,
        lock_digest: chosen.and_then(|record| record.lock_digest.clone()),
    };
    stamp(host, &mut placeholder);
    stop_check(host)?;
    if fresh {
        std::fs::create_dir_all(pool.slot_path(slot)).map_err(WorktreeError::from)?;
    }
    pool.write(&placeholder)?;
    let bases = if stale.is_empty() {
        Vec::new()
    } else {
        scan_output_bases(&host.output_roots(&host.servers()))
    };
    for slot in &stale {
        drop(delete_slot(pool, host, *slot, &bases));
    }
    drop(lock);
    let outcome = switch_slot(pool, host, slot, fresh, reclaimed, &start, branch, &target);
    let path = pool.slot_path(slot);
    if let Err(error) = outcome {
        if fresh {
            pool.remove(slot);
            drop(git::run(
                &pool.main,
                &[
                    OsStr::new("worktree"),
                    OsStr::new("remove"),
                    OsStr::new("--force"),
                    path.as_os_str(),
                ],
            ));
            if host.remove_tree(&path) {
            } else {
                Default::default()
            }
            drop(git::run(&pool.main, &["worktree", "prune"]));
        } else {
            let mut freed = placeholder.clone();
            freed.free();
            drop(pool.write(&freed));
            drop(git::run(
                &pool.main,
                &[
                    OsStr::new("worktree"),
                    OsStr::new("lock"),
                    OsStr::new("--reason"),
                    OsStr::new(FREE_LOCK_REASON),
                    path.as_os_str(),
                ],
            ));
            if takeover.is_none() && error.code == SLOT_UNCLEAN {
                return Err(Missed::Unclean {
                    slot,
                    error,
                    warnings,
                });
            }
        }
        return Err(error.into());
    }
    let mut record = placeholder;
    record.lock_digest = git::lock_digest(&path, "HEAD");
    pool.write(&record)?;
    delete_surplus(pool, host, slot, low_now);
    Ok(Added {
        slot,
        path,
        reused: false,
        base: match start {
            Start::Create(base) => Some(base),
            Start::Local | Start::Remote => None,
        },
        warnings,
        notice: None,
    })
}

const SLOT_UNCLEAN: &str = "slot_unclean";

fn clean_slot(host: &dyn Host, path: &Path) -> Result<(), WorktreeError> {
    const CLEAN: [&str; 4] = ["clean", "-ffdxq", "-e", "/bazel-*"];
    let unclean = |reason: String| {
        WorktreeError::refused(
            SLOT_UNCLEAN,
            &format!("cannot clean {}: {reason}", path.display()),
        )
    };
    if git::run(path, &CLEAN).is_ok() {
        return Ok(());
    }
    let listing = git::run(path, &["ls-files", "-z", "--others", "--directory"])
        .map_err(|error| unclean(error.message))?;
    for entry in listing
        .split('\0')
        .filter(|entry| !entry.is_empty() && !entry.starts_with("bazel-"))
    {
        let full = path.join(entry.trim_end_matches('/'));
        dx_process::make_writable(&full);
        if entry.ends_with('/') && !host.remove_tree(&full) {
            return Err(unclean(format!("{entry} cannot be deleted")));
        }
    }
    git::run(path, &CLEAN).map_err(|error| unclean(error.message))?;
    Ok(())
}

fn make_room(
    pool: &Pool,
    host: &dyn Host,
    branch: &str,
    reuse: Option<u32>,
) -> Result<Option<String>, WorktreeError> {
    let full = |space: Option<DiskSpace>| space.filter(|space| super::disk::is_full(space.free));
    let Some(mut space) = full(host.disk_space(&pool.base)) else {
        return Ok(None);
    };
    if let Some(slot) = reuse {
        let path = pool.slot_path(slot);
        let bases = scan_output_bases(&host.output_roots(&host.servers()));
        let owned: Vec<&OutputBase> = bases_of(&bases, &path)
            .into_iter()
            .filter(|base| disk_low(host, &base.dir))
            .collect();
        if !owned.is_empty() && !stop_server(host, &path) {
            let _: bool = drop_bases(host, &owned);
            let after = host.disk_space(&pool.base);
            if let Some(still) = full(after) {
                space = still
            } else {
                let free = after.map_or_else(|| "unknown".to_owned(), |after| size(after.free));
                return Ok(Some(format!(
                    "disk full: {} free; removed the output bases of slot {slot}, which this lease reuses; {free} free now",
                    size(space.free)
                )));
            }
        }
    }
    Err(WorktreeError::refused(
        "disk_full",
        &format!(
            "disk full: {} free at {}; cannot check out {branch}; free disk space with `dx worktree prune --apply --bases`",
            size(space.free),
            pool.base.display()
        ),
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct input"
)]
fn switch_slot(
    pool: &Pool,
    host: &dyn Host,
    slot: u32,
    fresh: bool,
    reclaimed: bool,
    start: &Start,
    branch: &str,
    target: &str,
) -> Result<(), WorktreeError> {
    let path = pool.slot_path(slot);
    stop_check(host)?;
    if fresh {
        std::fs::create_dir_all(&pool.base).map_err(WorktreeError::from)?;
        git::run(
            &pool.main,
            &[
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("--detach"),
                OsStr::new("--quiet"),
                path.as_os_str(),
                OsStr::new(target),
            ],
        )?;
    } else {
        if reclaimed {
            stop_server(host, &path);
            stop_check(host)?;
        }
        drop(git::run(
            &pool.main,
            &[
                OsStr::new("worktree"),
                OsStr::new("unlock"),
                path.as_os_str(),
            ],
        ));
        git::run(&path, &["reset", "--hard", "--quiet"])?;
        clean_slot(host, &path)?;
    }
    match start {
        Start::Create(base) => git::run(
            &path,
            &["switch", "--quiet", "-c", branch, "--no-track", base],
        )?,
        Start::Local => git::run(&path, &["switch", "--quiet", branch])?,
        Start::Remote => git::run(
            &path,
            &[
                "switch",
                "--quiet",
                "-c",
                branch,
                "--track",
                &format!("origin/{branch}"),
            ],
        )?,
    };
    copy_includes(&pool.main, &path)?;
    Ok(())
}

pub(crate) fn copy_includes(main: &Path, dest: &Path) -> Result<usize, WorktreeError> {
    if !main.join(".worktreeinclude").is_file() {
        return Ok(0);
    }
    let matched = git::run(
        main,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-from=.worktreeinclude",
        ],
    )?;
    if matched.is_empty() {
        return Ok(0);
    }
    let ignored =
        git::run_with_input(main, &["check-ignore", "-z", "--stdin"], matched.as_bytes())?;
    let mut copied = 0;
    for relative in ignored.split('\0').filter(|name| !name.is_empty()) {
        let from = main.join(relative);
        if !from.is_file() {
            continue;
        }
        let to = dest.join(relative);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(WorktreeError::from)?;
        }
        if relative == USER_BAZELRC {
            let text = std::fs::read_to_string(&from).map_err(WorktreeError::from)?;
            dx_atomic_fs::write_atomic(&to, without_output_base(&text).as_bytes())
                .map_err(WorktreeError::from)?;
        } else {
            std::fs::copy(&from, &to).map_err(WorktreeError::from)?;
        }
        copied += 1;
    }
    Ok(copied)
}

const USER_BAZELRC: &str = "user.bazelrc";

fn without_output_base(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| {
            let mut words = line.split_whitespace();
            let startup = words
                .next()
                .is_some_and(|command| command == "startup" || command.starts_with("startup:"));
            !(startup
                && words.any(|word| word == "--output_base" || word.starts_with("--output_base=")))
        })
        .collect()
}

pub fn claim(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    workspace: &Path,
) -> Result<Option<u32>, WorktreeError> {
    let Some(slot) = pool.slot_of(workspace) else {
        return Ok(None);
    };
    if !pool
        .record(slot)
        .is_some_and(|record| record.claimable_by(who))
    {
        return Ok(None);
    }
    let _lock = pool.lock()?;
    let Some(mut record) = pool.record(slot).filter(|record| record.claimable_by(who)) else {
        return Ok(None);
    };
    record.lease_to(who);
    stamp(host, &mut record);
    pool.write(&record)?;
    Ok(Some(slot))
}

pub fn release(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    path: &Path,
    force: bool,
) -> Result<Released, WorktreeError> {
    let mut released = match pool.slot_of(path) {
        Some(slot) => {
            let _lock = pool.lock()?;
            release_locked(pool, host, who, slot, None, force)?
        }
        None => release_external(pool, host, path, force)?,
    };
    released
        .warnings
        .extend(reclaim(pool, host, None).map(|reclaimed| reclaimed.warning));
    Ok(released)
}

pub(crate) fn release_lease(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    lease: &Record,
) -> Result<Released, WorktreeError> {
    let _lock = pool.lock()?;
    release_locked(pool, host, who, lease.slot, Some(lease), false)
}

fn orphan_record(pool: &Pool, slot: u32) -> Option<Record> {
    let path = pool.slot_path(slot);
    let checkout = pool
        .checkouts()
        .ok()?
        .into_iter()
        .find(|checkout| same_path(&checkout.path, &path))?;
    Some(Record {
        slot,
        held: true,
        branch: checkout.branch,
        last_branch: None,
        session: None,
        agent: None,
        pid: None,
        pid_start: None,
        boot_id: None,
        gone: false,
        stopped_at: None,
        unclaimed: false,
        broken: false,
        leased_at: 0,
        last_used: 0,
        lock_digest: None,
    })
}

fn gone_slot(pool: &Pool, slot: u32, path: PathBuf, branch: Option<String>) -> Released {
    pool.remove(slot);
    drop(git::run(
        &pool.main,
        &[
            OsStr::new("worktree"),
            OsStr::new("unlock"),
            path.as_os_str(),
        ],
    ));
    drop(git::run(&pool.main, &["worktree", "prune"]));
    Released {
        slot: Some(slot),
        path,
        branch,
        already_free: true,
        removed: false,
        warnings: Vec::new(),
    }
}

fn release_locked(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    slot: u32,
    expected: Option<&Record>,
    force: bool,
) -> Result<Released, WorktreeError> {
    let slot_path = pool.slot_path(slot);
    let Some(record) = pool.record(slot).or_else(|| orphan_record(pool, slot)) else {
        if !slot_path.exists() {
            return Ok(gone_slot(pool, slot, slot_path, None));
        }
        return Err(WorktreeError::usage(
            "not_a_slot",
            &format!("slot {slot} has no lease record; pass a path from dx worktree list"),
        ));
    };
    if expected.is_some_and(|lease| {
        record.held != lease.held
            || record.branch != lease.branch
            || record.session != lease.session
            || record.agent != lease.agent
            || record.leased_at != lease.leased_at
    }) {
        return Err(WorktreeError::refused(
            "lease_changed",
            &format!("slot {slot} was leased again; run dx worktree list to see its holder"),
        ));
    }
    if !slot_path.exists() {
        return Ok(gone_slot(pool, slot, slot_path, record.branch));
    }
    if !record.held {
        return Ok(Released {
            slot: Some(slot),
            path: slot_path,
            branch: None,
            already_free: true,
            removed: false,
            warnings: Vec::new(),
        });
    }
    let Some(_quiet) = pool.try_quiet(slot)? else {
        return Err(busy_error(pool, slot));
    };
    let reclaimable = force
        || record.held_by(who)
        || matches!(hold_known(pool, host, &record, false), Hold::Reclaimable);
    stop_check(host)?;
    if !reclaimable {
        return Err(WorktreeError::usage(
            "slot_leased",
            &format!(
                "slot {slot} is leased to {}; only that holder can release it unless it is reclaimable, or pass --force",
                record
                    .holder(holder_alive(host, &record))
                    .unwrap_or_else(|| "nobody".to_owned())
            ),
        ));
    }
    if !force {
        if git::dirty(&slot_path, &|| host.interrupted())? {
            return Err(WorktreeError::refused(
                "dirty_slot",
                &format!(
                    "slot {slot} at {} has uncommitted changes; commit them or pass --force",
                    slot_path.display()
                ),
            ));
        }
        let ahead = git::unpushed(&slot_path, &|| host.interrupted())?;
        if ahead > 0 {
            return Err(WorktreeError::refused(
                "unpushed_commits",
                &format!(
                    "slot {slot} at {} has {ahead} commit(s) on no remote; push them or pass --force",
                    slot_path.display()
                ),
            ));
        }
    }
    stop_server(host, &slot_path);
    stop_check(host)?;
    git::run(&slot_path, &["switch", "--quiet", "--detach"])?;
    drop(git::run(
        &pool.main,
        &[
            OsStr::new("worktree"),
            OsStr::new("lock"),
            OsStr::new("--reason"),
            OsStr::new(FREE_LOCK_REASON),
            slot_path.as_os_str(),
        ],
    ));
    let mut freed = record.clone();
    freed.last_branch = record.branch.clone().or(record.last_branch.clone());
    freed.free();
    freed.last_used = host.now();
    pool.write(&freed)?;
    Ok(Released {
        slot: Some(slot),
        path: slot_path,
        branch: record.branch,
        already_free: false,
        removed: false,
        warnings: Vec::new(),
    })
}

fn external_checkout(pool: &Pool, path: &Path) -> Option<git::Checkout> {
    pool.checkouts().ok()?.into_iter().find(|checkout| {
        !same_path(&checkout.path, &pool.main)
            && pool.slot_of(&checkout.path).is_none()
            && same_path(&checkout.path, path)
    })
}

fn bases_of<'a>(bases: &'a [OutputBase], path: &Path) -> Vec<&'a OutputBase> {
    bases
        .iter()
        .filter(|base| same_path(&base.workspace, path))
        .collect()
}

fn teardown(
    pool: &Pool,
    host: &dyn Host,
    path: &Path,
    owned: Vec<&OutputBase>,
) -> Result<(bool, Vec<OutputBase>), WorktreeError> {
    stop_check(host)?;
    stop_server(host, path);
    stop_check(host)?;
    let arg = path.as_os_str();
    drop(git::run(
        &pool.main,
        &[OsStr::new("worktree"), OsStr::new("unlock"), arg],
    ));
    drop(git::run(
        &pool.main,
        &[
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            arg,
        ],
    ));
    let gone = host.remove_tree(path);
    drop(git::run(&pool.main, &["worktree", "prune"]));
    let stuck = owned
        .into_iter()
        .filter(|base| !host.remove_tree(&base.dir))
        .cloned()
        .collect();
    Ok((gone, stuck))
}

fn remove_external(
    pool: &Pool,
    host: &dyn Host,
    path: &Path,
    bases: &[OutputBase],
) -> Result<bool, WorktreeError> {
    let (gone, stuck) = teardown(pool, host, path, bases_of(bases, path))?;
    Ok(gone && stuck.is_empty())
}

fn release_external(
    pool: &Pool,
    host: &dyn Host,
    path: &Path,
    force: bool,
) -> Result<Released, WorktreeError> {
    let checkout = external_checkout(pool, path).ok_or_else(|| {
        WorktreeError::usage(
            "not_a_slot",
            &format!(
                "{} is not a dx worktree slot or a worktree of this repository; pass a path from dx worktree list",
                path.display()
            ),
        )
    })?;
    let _lock = pool.lock()?;
    let bases = scan_output_bases(&host.output_roots(&host.servers()));
    if !force {
        if git::dirty(&checkout.path, &|| host.interrupted())? {
            return Err(WorktreeError::refused(
                "dirty_slot",
                &format!(
                    "worktree {} has uncommitted changes; commit them or pass --force",
                    checkout.path.display()
                ),
            ));
        }
        let ahead = git::unpushed(&checkout.path, &|| host.interrupted())?;
        if ahead > 0 {
            return Err(WorktreeError::refused(
                "unpushed_commits",
                &format!(
                    "worktree {} has {ahead} commit(s) on no remote; push them or pass --force",
                    checkout.path.display()
                ),
            ));
        }
    }
    if !remove_external(pool, host, &checkout.path, &bases)? {
        return Err(WorktreeError::refused(
            "busy",
            &format!(
                "worktree {} could not be fully removed and is still on disk; its servers and git registration may already be gone, so close what holds it and run release again",
                checkout.path.display()
            ),
        ));
    }
    Ok(Released {
        slot: None,
        path: checkout.path,
        branch: checkout.branch,
        already_free: false,
        removed: true,
        warnings: Vec::new(),
    })
}

fn external_plan_reason(
    pool: &Pool,
    host: &dyn Host,
    checkout: &git::Checkout,
    base: Option<&str>,
    servers: &[Server],
) -> Option<&'static str> {
    if let Some(reason) = external_keep_reason(host, checkout, servers) {
        return Some(reason);
    }
    let path = &checkout.path;
    let merged = || base.is_some_and(|base| git::is_ancestor(path, "HEAD", base));
    match checkout.branch.as_deref() {
        Some(branch) => {
            if !git::tracks_own_remote(&pool.main, branch) {
                Some("local")
            } else if merged() || git::upstream_gone(&pool.main, branch) {
                None
            } else {
                Some("unmerged")
            }
        }
        None => {
            if !merged() {
                Some("unmerged")
            } else if checkout_recent(host, path) {
                Some("recent")
            } else {
                None
            }
        }
    }
}

fn checkout_recent(host: &dyn Host, checkout: &Path) -> bool {
    let Some(admin) = git::admin_dir(checkout) else {
        return true;
    };
    let newest = ["HEAD", "index", "logs/HEAD"]
        .iter()
        .filter_map(|name| file_modified(&admin.join(name)))
        .max();
    newest.is_none_or(|used| host.now().saturating_sub(used) < EXTERNAL_GRACE_SECS)
}

fn file_modified(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

fn external_keep_reason(
    host: &dyn Host,
    checkout: &git::Checkout,
    servers: &[Server],
) -> Option<&'static str> {
    let path = &checkout.path;
    if checkout.locked {
        Some("locked")
    } else if server_in(servers, path) {
        Some("server")
    } else if !matches!(git::dirty(path, &|| host.interrupted()), Ok(false)) {
        Some("dirty")
    } else if !matches!(git::unpushed(path, &|| host.interrupted()), Ok(0)) {
        Some("unpushed")
    } else {
        None
    }
}

fn server_in(servers: &[Server], path: &Path) -> bool {
    servers.iter().any(|server| {
        server
            .workspace
            .as_deref()
            .is_some_and(|workspace| same_path(workspace, path))
    })
}

pub(crate) fn stop_server(host: &dyn Host, path: &Path) -> bool {
    if !host.servers_known() {
        return !host.bazel_shutdown(path);
    }
    if !server_in(&host.servers(), path) {
        return false;
    }
    host.bazel_shutdown(path);
    server_in(&host.servers(), path)
}

pub fn list(pool: &Pool, host: &dyn Host, disk: bool) -> Result<Listing, WorktreeError> {
    pool.forget_missing();
    let servers = host.servers();
    let bases = if disk {
        scan_output_bases(&host.output_roots(&servers))
    } else {
        Vec::new()
    };
    let mut rows = Vec::new();
    let mut slot_disk = 0;
    let records = pool.records();
    let now = host.now();
    let mut prunable: Vec<u32> = records
        .iter()
        .filter(|record| !record.held && now.saturating_sub(record.last_used) >= STALE_AFTER_SECS)
        .map(|record| record.slot)
        .collect();
    if disk {
        let surplus = surplus_slots(
            pool,
            host,
            &records,
            &SurplusLimit {
                keep: 0,
                total: records
                    .iter()
                    .filter(|record| !record.broken && !prunable.contains(&record.slot))
                    .count(),
                low_disk: false,
            },
        );
        for number in surplus {
            if !prunable.contains(&number) {
                prunable.push(number);
            }
        }
    }
    for record in records {
        let path = pool.slot_path(record.slot);
        let busy = pool.busy(record.slot);
        let held = hold_known(pool, host, &record, record.held && busy);
        stop_check(host)?;
        let state = match held {
            Hold::Free => State::Free,
            Hold::Reclaimable => State::Reclaimable,
            Hold::Alive | Hold::Waiting(_) => State::Held,
        };
        let disk_bytes = if disk {
            let mut bytes = measure(host, &path)?;
            for base in bases
                .iter()
                .filter(|base| same_path(&base.workspace, &path))
            {
                bytes += measure(host, &base.dir)?;
            }
            Some(bytes)
        } else {
            None
        };
        slot_disk += disk_bytes.unwrap_or(0);
        rows.push(Row {
            slot: Some(record.slot),
            branch: git::current_branch(&path).or(record.branch.clone()),
            holder: record.holder(matches!(held, Hold::Alive)),
            state,
            reason: match held {
                Hold::Waiting(reason) => Some(reason),
                Hold::Free | Hold::Alive | Hold::Reclaimable => None,
            },
            server: server_in(&servers, &path),
            busy,
            disk_bytes,
            prunable: prunable.contains(&record.slot),
            path,
        });
    }
    for checkout in pool.checkouts()? {
        if same_path(&checkout.path, &pool.main) || pool.slot_of(&checkout.path).is_some() {
            continue;
        }
        let disk_bytes = if disk {
            Some(measure(host, &checkout.path)?)
        } else {
            None
        };
        rows.push(Row {
            slot: None,
            branch: checkout.branch.clone(),
            holder: None,
            state: State::External,
            reason: None,
            server: server_in(&servers, &checkout.path),
            busy: false,
            disk_bytes,
            prunable: false,
            path: checkout.path,
        });
    }
    let disk_cache_bytes = if disk {
        Some(disk_cache_bytes(host)?)
    } else {
        None
    };
    Ok(Listing {
        rows,
        servers: servers.len() as u64,
        servers_rss_bytes: servers.iter().map(|server| server.rss_bytes).sum(),
        disk_bytes: disk.then_some(slot_disk),
        disk_cache_bytes,
    })
}

pub fn disk_cache_bytes(host: &dyn Host) -> Result<u64, WorktreeError> {
    match host.disk_cache().filter(|dir| dir.is_dir()) {
        Some(dir) => measure(host, &dir),
        None => Ok(0),
    }
}

fn stop_check(host: &dyn Host) -> Result<(), WorktreeError> {
    match host.interrupted() {
        Some(signo) => Err(WorktreeError::stopped(signo)),
        None => Ok(()),
    }
}

fn measure(host: &dyn Host, path: &Path) -> Result<u64, WorktreeError> {
    stop_check(host)?;
    let bytes = host.disk_bytes(path);
    stop_check(host)?;
    Ok(bytes)
}

pub fn path_of(pool: &Pool, branch: &str) -> Result<PathBuf, WorktreeError> {
    pool.forget_missing();
    if let Some(record) = pool
        .records()
        .iter()
        .find(|record| record.held && record.branch.as_deref() == Some(branch))
    {
        return Ok(pool.slot_path(record.slot));
    }
    for checkout in pool.checkouts()? {
        if checkout.branch.as_deref() == Some(branch) {
            return Ok(checkout.path);
        }
    }
    Err(WorktreeError::refused(
        "branch_not_held",
        &format!("no checkout holds branch {branch}; run dx worktree add {branch}"),
    ))
}

pub fn prune(
    pool: &Pool,
    host: &dyn Host,
    apply: bool,
    bases: bool,
) -> Result<Pruned, WorktreeError> {
    let trigger = if bases {
        Trigger::OnDemand
    } else {
        Trigger::LowDisk
    };
    if !apply {
        return Ok(Pruned {
            plan: prune_plan(pool, host, trigger)?,
            ..Pruned::default()
        });
    }
    let lock = pool.lock()?;
    let mut plan = prune_plan(pool, host, trigger)?;
    let Applied {
        kept,
        skipped_slots,
        skipped_bases,
        skipped_caches,
        used_downloads,
    } = prune_apply(pool, host, &plan, &lock)?;
    plan.caches
        .retain(|cache| !skipped_caches.iter().any(|item| item.path == cache.path));
    plan.externals
        .retain(|external| !kept.iter().any(|item| item.path == external.path));
    plan.slots
        .retain(|slot| !skipped_slots.iter().any(|item| item.slot == slot.slot));
    plan.bases
        .retain(|base| !skipped_bases.iter().any(|item| item.dir == base.dir));
    if !used_downloads.is_empty() {
        for downloads in &mut plan.downloads {
            downloads
                .entries
                .retain(|entry| !used_downloads.contains(&entry.dir));
            downloads.bytes = downloads.entries.iter().map(|entry| entry.bytes).sum();
        }
        plan.downloads
            .retain(|downloads| !downloads.entries.is_empty());
    }
    for slot in &mut plan.slots {
        for base in skipped_bases
            .iter()
            .filter(|base| same_path(&base.workspace, &slot.path))
        {
            slot.bytes = slot.bytes.saturating_sub(measure(host, &base.dir)?);
        }
    }
    Ok(Pruned {
        plan,
        kept,
        skipped_slots,
        skipped_bases,
        skipped_caches,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    LowDisk,
    OnDemand,
}

fn free_slot_bases<'a>(
    pool: &Pool,
    host: &dyn Host,
    bases: &'a [OutputBase],
    trigger: Trigger,
    skip: impl Fn(&Record) -> bool,
) -> Vec<(u32, Vec<&'a OutputBase>)> {
    let mut free: Vec<Record> = pool
        .records()
        .into_iter()
        .filter(|record| !record.held && !skip(record))
        .collect();
    free.sort_by_key(|record| (record.last_used, record.slot));
    free.into_iter()
        .filter_map(|record| {
            let owned: Vec<&OutputBase> = bases_of(bases, &pool.slot_path(record.slot))
                .into_iter()
                .filter(|base| trigger == Trigger::OnDemand || disk_low(host, &base.dir))
                .collect();
            (!owned.is_empty()).then_some((record.slot, owned))
        })
        .collect()
}

pub(crate) fn prune_plan(
    pool: &Pool,
    host: &dyn Host,
    trigger: Trigger,
) -> Result<PrunePlan, WorktreeError> {
    pool.forget_missing();
    let now = host.now();
    let servers = host.servers();
    let roots = host.output_roots(&servers);
    let bases = scan_output_bases(&roots);
    let mut plan = PrunePlan::default();
    for record in pool.records() {
        let idle = now.saturating_sub(record.last_used);
        if record.held || (!record.broken && idle < STALE_AFTER_SECS) {
            continue;
        }
        let path = pool.slot_path(record.slot);
        let mut bytes = measure(host, &path)?;
        for base in bases
            .iter()
            .filter(|base| same_path(&base.workspace, &path))
        {
            bytes += measure(host, &base.dir)?;
        }
        plan.slots.push(PruneSlot {
            slot: record.slot,
            path,
            idle_days: idle / 86_400,
            bytes,
        });
    }
    let default = git::default_base(&pool.main).ok();
    for checkout in pool.checkouts().unwrap_or_default() {
        if same_path(&checkout.path, &pool.main) || pool.slot_of(&checkout.path).is_some() {
            continue;
        }
        if let Some(reason) =
            external_plan_reason(pool, host, &checkout, default.as_deref(), &servers)
        {
            plan.kept_externals.push(PruneKept {
                path: checkout.path,
                branch: checkout.branch,
                reason,
            });
            continue;
        }
        let mut bytes = measure(host, &checkout.path)?;
        for base in bases
            .iter()
            .filter(|base| same_path(&base.workspace, &checkout.path))
        {
            bytes += measure(host, &base.dir)?;
        }
        plan.externals.push(PruneExternal {
            path: checkout.path,
            branch: checkout.branch,
            bytes,
        });
    }
    for base in &bases {
        if base.workspace.exists() {
            continue;
        }
        if base_in_use(&servers, base) {
            plan.kept_bases.push(PruneKeptBase {
                dir: base.dir.clone(),
                workspace: base.workspace.clone(),
                owner: BaseOwner::Orphan,
                reason: "server",
            });
            continue;
        }
        plan.bases.push(PruneBase {
            dir: base.dir.clone(),
            workspace: base.workspace.clone(),
            bytes: measure(host, &base.dir)?,
            owner: BaseOwner::Orphan,
        });
    }
    plan.downloads = stale_downloads(host, &roots, now)?;
    let listed = free_slot_bases(pool, host, &bases, trigger, |record| {
        plan.slots.iter().any(|slot| slot.slot == record.slot)
    });
    for (slot, owned) in listed {
        for base in owned {
            plan.bases.push(PruneBase {
                dir: base.dir.clone(),
                workspace: base.workspace.clone(),
                bytes: measure(host, &base.dir)?,
                owner: BaseOwner::Slot(slot),
            });
        }
    }
    let mut free: Vec<Record> = pool
        .records()
        .into_iter()
        .filter(|record| !record.held && !plan.slots.iter().any(|slot| slot.slot == record.slot))
        .collect();
    free.sort_by_key(|record| (record.last_used, record.slot));
    for record in free {
        for path in cache::slot_caches(&pool.slot_path(record.slot)) {
            if trigger == Trigger::LowDisk && !disk_low(host, &path) {
                continue;
            }
            plan.caches.push(PruneCache {
                slot: record.slot,
                bytes: measure(host, &path)?,
                path,
            });
        }
    }
    Ok(plan)
}

fn base_in_use(servers: &[Server], base: &OutputBase) -> bool {
    servers.iter().any(|server| {
        server
            .output_base
            .as_deref()
            .is_some_and(|dir| same_path(dir, &base.dir))
    })
}

pub(crate) fn prune_apply(
    pool: &Pool,
    host: &dyn Host,
    plan: &PrunePlan,
    _lock: &dx_atomic_fs::lease::ExclusiveLease,
) -> Result<Applied, WorktreeError> {
    let bases = scan_output_bases(&host.output_roots(&host.servers()));
    let mut skipped_slots = Vec::new();
    let mut skipped = Vec::new();
    for slot in &plan.slots {
        stop_check(host)?;
        let Some(record) = pool.record(slot.slot) else {
            continue;
        };
        let left = if record.held {
            Leftover {
                slot: Some("held"),
                bases: Vec::new(),
            }
        } else {
            delete_slot(pool, host, slot.slot, &bases)?
        };
        if let Some(reason) = left.slot {
            skipped_slots.push(PruneSkippedSlot {
                slot: slot.slot,
                path: slot.path.clone(),
                reason,
            });
        }
        skipped.extend(left.bases.into_iter().map(|base| PruneKeptBase {
            dir: base.dir,
            workspace: base.workspace,
            owner: BaseOwner::Orphan,
            reason: "busy",
        }));
    }
    let servers = host.servers();
    let mut kept = Vec::new();
    for external in &plan.externals {
        let Some(checkout) = external_checkout(pool, &external.path) else {
            continue;
        };
        stop_check(host)?;
        if let Some(reason) = external_keep_reason(host, &checkout, &servers) {
            kept.push(PruneKept {
                path: checkout.path,
                branch: checkout.branch,
                reason,
            });
            continue;
        }
        if !remove_external(pool, host, &external.path, &bases)? {
            kept.push(PruneKept {
                path: checkout.path,
                branch: checkout.branch,
                reason: "busy",
            });
        }
    }
    for base in &plan.bases {
        stop_check(host)?;
        let skip = |reason| PruneKeptBase {
            dir: base.dir.clone(),
            workspace: base.workspace.clone(),
            owner: base.owner.clone(),
            reason,
        };
        if let Some(slot) = base.owner.slot() {
            if pool.record(slot).is_none_or(|record| record.held) {
                skipped.push(skip("held"));
                continue;
            }
            if pool.busy(slot) {
                skipped.push(skip("busy"));
                continue;
            }
            let path = pool.slot_path(slot);
            if stop_server(host, &path) {
                skipped.push(skip("server"));
                continue;
            }
        }
        if !host.remove_tree(&base.dir) {
            skipped.push(skip("busy"));
        }
    }
    let mut skipped_caches = Vec::new();
    for cache in &plan.caches {
        stop_check(host)?;
        let reason = if pool.record(cache.slot).is_none_or(|record| record.held) {
            Some("held")
        } else if pool.busy(cache.slot) || !host.remove_tree(&cache.path) {
            Some("busy")
        } else {
            None
        };
        if let Some(reason) = reason {
            skipped_caches.push(PruneSkippedCache {
                slot: cache.slot,
                path: cache.path.clone(),
                reason,
            });
        }
    }
    let mut used_downloads = Vec::new();
    for entry in plan
        .downloads
        .iter()
        .flat_map(|downloads| &downloads.entries)
    {
        stop_check(host)?;
        if !drop_download(host, entry) {
            used_downloads.push(entry.dir.clone());
        }
    }
    Ok(Applied {
        kept,
        skipped_slots,
        skipped_bases: skipped,
        skipped_caches,
        used_downloads,
    })
}

fn stale_downloads(
    host: &dyn Host,
    roots: &[PathBuf],
    now: u64,
) -> Result<Vec<PruneDownloads>, WorktreeError> {
    stop_check(host)?;
    let found = scan_downloads(roots, &|| host.interrupted());
    stop_check(host)?;
    let mut groups: Vec<PruneDownloads> = Vec::new();
    for entry in found
        .into_iter()
        .filter(|entry| now.saturating_sub(entry.used) >= STALE_AFTER_SECS)
    {
        match groups.iter_mut().find(|group| group.cache == entry.cache) {
            Some(group) => {
                group.bytes += entry.bytes;
                group.entries.push(entry);
            }
            None => groups.push(PruneDownloads {
                cache: entry.cache.clone(),
                bytes: entry.bytes,
                entries: vec![entry],
            }),
        }
    }
    Ok(groups)
}

fn drop_download(host: &dyn Host, entry: &Download) -> bool {
    let Some(used) = download_used(&entry.dir) else {
        return false;
    };
    if host.now().saturating_sub(used) < STALE_AFTER_SECS {
        return false;
    }
    host.remove_tree(&entry.dir)
}

fn is_low(space: DiskSpace) -> bool {
    super::disk::is_low(space.free, space.total)
}

pub(crate) fn disk_low(host: &dyn Host, path: &Path) -> bool {
    host.disk_space(path).is_some_and(is_low)
}

pub(crate) fn low_disk(host: &dyn Host, base: &Path) -> Option<(PathBuf, DiskSpace)> {
    low_disk_reading(host, base).1
}

fn low_disk_reading(
    host: &dyn Host,
    base: &Path,
) -> (Option<DiskSpace>, Option<(PathBuf, DiskSpace)>) {
    let Some(space) = host.disk_space(base) else {
        return (None, None);
    };
    if is_low(space) {
        return (Some(space), Some((base.to_path_buf(), space)));
    }
    let lowest = |roots: Vec<PathBuf>| {
        roots
            .into_iter()
            .filter(|root| root.is_dir())
            .filter_map(|root| {
                host.disk_space(&root)
                    .filter(|space| is_low(*space))
                    .map(|space| (root, space))
            })
            .min_by_key(|(_, space)| space.free)
    };
    if lowest(host.config_output_roots()).is_none() {
        return (Some(space), None);
    }
    (Some(space), lowest(host.output_roots(&host.servers())))
}

fn drop_bases(host: &dyn Host, owned: &[&OutputBase]) -> bool {
    let mut gone = true;
    for base in owned {
        gone &= host.remove_tree(&base.dir);
    }
    gone
}

pub(crate) fn reclaim(pool: &Pool, host: &dyn Host, keep: Option<u32>) -> Option<Reclaim> {
    match reclaim_if_low(pool, host, keep) {
        Ok(reclaimed) => reclaimed,
        Err(error) => Some(Reclaim {
            warning: reclaim_failed(&error),
            removed: false,
            skipped: false,
        }),
    }
}

pub(crate) fn reclaim_failed(error: &WorktreeError) -> Warning {
    Warning {
        code: "disk_low",
        message: format!("low disk, cannot reclaim output bases: {error}"),
    }
}

pub(crate) fn reclaim_if_low(
    pool: &Pool,
    host: &dyn Host,
    keep: Option<u32>,
) -> Result<Option<Reclaim>, WorktreeError> {
    let Some(low) = low_disk(host, &pool.base) else {
        return Ok(None);
    };
    let lock = pool.lock()?;
    pool.forget_missing();
    Ok(Some(reclaim_locked(pool, host, keep, low, &lock)))
}

fn reclaim_locked(
    pool: &Pool,
    host: &dyn Host,
    keep: Option<u32>,
    low: (PathBuf, DiskSpace),
    _lock: &dx_atomic_fs::lease::ExclusiveLease,
) -> Reclaim {
    reclaim_host_wide(Some((pool, keep)), host, low)
}

pub(crate) fn reclaim_unpooled(host: &dyn Host, low: (PathBuf, DiskSpace)) -> Reclaim {
    reclaim_host_wide(None, host, low)
}

fn reclaim_host_wide(
    slots_of: Option<(&Pool, Option<u32>)>,
    host: &dyn Host,
    (disk, before): (PathBuf, DiskSpace),
) -> Reclaim {
    let servers = host.servers();
    let roots = host.output_roots(&servers);
    let bases = scan_output_bases(&roots);
    let mut orphans = 0usize;
    let mut slots: Vec<u32> = Vec::new();
    let mut kept: Vec<u32> = Vec::new();
    for base in bases
        .iter()
        .filter(|base| !base.workspace.exists() && !base_in_use(&servers, base))
    {
        if !disk_low(host, &base.dir) {
            continue;
        }
        if drop_bases(host, &[base]) {
            orphans += 1;
        }
    }
    let now = host.now();
    let mut stale: Vec<Download> = scan_downloads(&roots, &|| host.interrupted())
        .into_iter()
        .filter(|entry| now.saturating_sub(entry.used) >= STALE_AFTER_SECS)
        .collect();
    stale.sort_by(|left, right| (left.used, &left.dir).cmp(&(right.used, &right.dir)));
    let mut downloads = 0usize;
    for entry in &stale {
        if host.interrupted().is_some() || !disk_low(host, &entry.cache) {
            continue;
        }
        if drop_download(host, entry) {
            downloads += 1;
        }
    }
    if let Some((pool, keep)) = slots_of {
        let picked = free_slot_bases(pool, host, &bases, Trigger::LowDisk, |record| {
            Some(record.slot) == keep || pool.busy(record.slot)
        });
        for (slot, owned) in picked {
            let path = pool.slot_path(slot);
            if stop_server(host, &path) {
                kept.push(slot);
                continue;
            }
            if drop_bases(host, &owned) {
                slots.push(slot);
            }
        }
    }
    let after = host.disk_space(&disk).unwrap_or(before);
    let mut message = format!("low disk at {}: {} free", disk.display(), size(before.free));
    let mut removed = Vec::new();
    if orphans > 0 {
        removed.push(format!("{orphans} orphaned output base(s)"));
    }
    if downloads > 0 {
        removed.push(format!("{downloads} repository cache download(s)"));
    }
    if !slots.is_empty() {
        let numbers: Vec<String> = slots.iter().map(u32::to_string).collect();
        removed.push(format!(
            "the output bases of free slot(s) {}",
            numbers.join(", ")
        ));
    }
    let reclaimed = !removed.is_empty();
    if reclaimed {
        message.push_str(&format!(
            "; removed {}; {} free now",
            removed.join(" and "),
            size(after.free)
        ));
    } else {
        message.push_str("; nothing to reclaim");
    }
    if !kept.is_empty() {
        let numbers: Vec<String> = kept.iter().map(u32::to_string).collect();
        message.push_str(&format!(
            "; kept the output bases of free slot(s) {} (server would not stop)",
            numbers.join(", ")
        ));
    }
    if is_low(after) {
        message.push_str("; run dx worktree prune --apply");
    }
    Reclaim {
        warning: Warning {
            code: "disk_low",
            message,
        },
        removed: reclaimed,
        skipped: !kept.is_empty(),
    }
}

#[must_use]
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!(
            "{value:.1} {}",
            UNITS.get(unit).copied().unwrap_or_default()
        )
    }
}

#[derive(Debug, Default)]
struct Leftover {
    slot: Option<&'static str>,
    bases: Vec<OutputBase>,
}

fn delete_slot(
    pool: &Pool,
    host: &dyn Host,
    slot: u32,
    bases: &[OutputBase],
) -> Result<Leftover, WorktreeError> {
    let Ok(Some(_quiet)) = pool.try_quiet(slot) else {
        return Ok(Leftover {
            slot: Some("busy"),
            bases: Vec::new(),
        });
    };
    let path = pool.slot_path(slot);
    let (gone, stuck) = teardown(pool, host, &path, bases_of(bases, &path))?;
    if gone {
        pool.remove(slot);
        return Ok(Leftover {
            slot: None,
            bases: stuck,
        });
    }
    mark_broken(pool, slot);
    Ok(Leftover {
        slot: Some("busy"),
        bases: Vec::new(),
    })
}

fn mark_broken(pool: &Pool, slot: u32) {
    let Some(mut record) = pool.record(slot) else {
        return;
    };
    record.broken = true;
    if pool.write(&record).is_err() {
        pool.remove(slot);
    }
}
