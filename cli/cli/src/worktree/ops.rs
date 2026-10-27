use std::path::{Path, PathBuf};

use dx_path::nested_bazel;

use super::git;
use super::host::{
    download_used, resolved, same_path, scan_downloads, scan_output_bases, DiskSpace, Download,
    Host, OutputBase, Server,
};
use super::pool::{Pool, Record, Who};
use super::WorktreeError;

const STALE_AFTER_SECS: u64 = 30 * 24 * 60 * 60;
const EXTERNAL_GRACE_SECS: u64 = 24 * 60 * 60;
const FREE_LOCK_REASON: &str = "dx: free";

/// What a slot is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Held,
    Free,
    Reclaimable,
    External,
}

impl State {
    pub(crate) fn name(self) -> &'static str {
        match self {
            State::Held => "held",
            State::Free => "free",
            State::Reclaimable => "reclaimable",
            State::External => "external",
        }
    }
}

/// Where a new branch starts.
enum Start {
    Create(String),
    Local,
    Remote,
}

/// A request to lease a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddRequest {
    pub(crate) branch: String,
    pub(crate) create: Option<String>,
    pub(crate) force: bool,
    pub(crate) unclaimed: bool,
}

/// The slot an `add` handed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Added {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) reused: bool,
    pub(crate) base: Option<String>,
    pub(crate) warnings: Vec<Warning>,
}

/// A problem `add` worked around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Warning {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

/// What a low-disk reclaim did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reclaim {
    pub(crate) warning: Warning,
    pub(crate) removed: bool,
}

/// The slot a `release` freed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Released {
    pub(crate) slot: Option<u32>,
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) already_free: bool,
    pub(crate) removed: bool,
    pub(crate) warnings: Vec<Warning>,
}

/// One row of `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) slot: Option<u32>,
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) holder: Option<String>,
    pub(crate) state: State,
    pub(crate) reason: Option<&'static str>,
    pub(crate) server: bool,
    pub(crate) disk_bytes: Option<u64>,
}

/// The slots plus the machine totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listing {
    pub(crate) rows: Vec<Row>,
    pub(crate) servers: u64,
    pub(crate) servers_rss_bytes: u64,
    pub(crate) disk_bytes: Option<u64>,
    pub(crate) disk_cache_bytes: Option<u64>,
}

/// One slot `prune` would delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneSlot {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) idle_days: u64,
    pub(crate) bytes: u64,
}

/// Whose output base `prune` lists, keeps or skips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BaseOwner {
    /// Its workspace is gone.
    Orphan,
    /// A free slot of the pool.
    Slot(u32),
    /// A test case in a free nested-Bazel slot; `slot` is the slot directory and `case` the workspace name.
    Nested { slot: PathBuf, case: String },
}

impl BaseOwner {
    /// The pool slot, if a pool slot owns the base.
    pub(crate) fn slot(&self) -> Option<u32> {
        match self {
            BaseOwner::Slot(slot) => Some(*slot),
            _ => None,
        }
    }
}

/// One output base whose workspace is gone, of a free slot when its disk is low, or of a nested-Bazel test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) bytes: u64,
    pub(crate) owner: BaseOwner,
}

/// One output base `prune` keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneKeptBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) owner: BaseOwner,
    pub(crate) reason: &'static str,
}

/// One external worktree `prune` would remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneExternal {
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) bytes: u64,
}

/// One external worktree `prune` keeps, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneKept {
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) reason: &'static str,
}

/// The downloads in one repository cache that no build used for 30 days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneDownloads {
    pub(crate) cache: PathBuf,
    pub(crate) entries: Vec<Download>,
    pub(crate) bytes: u64,
}

/// One slot `prune --apply` could not delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneSkippedSlot {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) reason: &'static str,
}

/// What `prune_apply` kept or could not delete.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Applied {
    pub(crate) kept: Vec<PruneKept>,
    pub(crate) skipped_slots: Vec<PruneSkippedSlot>,
    pub(crate) skipped_bases: Vec<PruneKeptBase>,
    pub(crate) used_downloads: Vec<PathBuf>,
}

/// What `prune` deleted or would delete, and what it kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Pruned {
    pub(crate) plan: PrunePlan,
    pub(crate) kept: Vec<PruneKept>,
    pub(crate) skipped_slots: Vec<PruneSkippedSlot>,
    pub(crate) skipped_bases: Vec<PruneKeptBase>,
}

/// What `prune` found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PrunePlan {
    pub(crate) slots: Vec<PruneSlot>,
    pub(crate) externals: Vec<PruneExternal>,
    pub(crate) bases: Vec<PruneBase>,
    pub(crate) kept_bases: Vec<PruneKeptBase>,
    pub(crate) kept_externals: Vec<PruneKept>,
    pub(crate) downloads: Vec<PruneDownloads>,
}

impl PrunePlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.slots.is_empty()
            && self.externals.is_empty()
            && self.bases.is_empty()
            && self.downloads.is_empty()
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.slots.iter().map(|slot| slot.bytes).sum::<u64>()
            + self
                .externals
                .iter()
                .map(|external| external.bytes)
                .sum::<u64>()
            + self.bases.iter().map(|base| base.bytes).sum::<u64>()
            + self
                .downloads
                .iter()
                .map(|downloads| downloads.bytes)
                .sum::<u64>()
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn holder_alive(host: &dyn Host, record: &Record) -> bool {
    !record.gone && record.pid.is_some_and(|pid| host.process_alive(pid))
}

/// How a slot record is held.
enum Hold {
    Free,
    Alive,
    Reclaimable,
    Waiting(&'static str),
}

/// Classifies a slot record, checking its holder and git state once.
fn hold(pool: &Pool, host: &dyn Host, record: &Record) -> Hold {
    hold_known(pool, host, record, record.held && pool.busy(record.slot))
}

fn hold_known(pool: &Pool, host: &dyn Host, record: &Record, busy: bool) -> Hold {
    if !record.held {
        return Hold::Free;
    }
    if busy || holder_alive(host, record) {
        return Hold::Alive;
    }
    let path = pool.slot_path(record.slot);
    if !matches!(git::dirty(&path), Ok(false)) {
        return Hold::Waiting("holder gone, tree has uncommitted changes");
    }
    if !matches!(git::unpushed(&path), Ok(0)) {
        return Hold::Waiting("holder gone, commits are not pushed");
    }
    Hold::Reclaimable
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
                &format!("--create base {requested} does not name a commit; pass --create=origin/<branch> or a commit"),
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

/// The first of `ranked` with no dx command running, locked into `quiet`.
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

/// Finds the held record on a branch.
fn held_on<'a>(records: &'a [Record], branch: &str) -> Option<&'a Record> {
    records
        .iter()
        .find(|record| record.held && record.branch.as_deref() == Some(branch))
}

/// Returns the slot the caller already holds on a branch, before any fetch.
fn reuse_held(pool: &Pool, who: &Who, branch: &str) -> Result<Option<Added>, WorktreeError> {
    let _lock = pool.lock()?;
    pool.forget_missing();
    let records = pool.records();
    Ok(held_on(&records, branch)
        .filter(|record| record.held_by(who))
        .map(|record| Added {
            slot: record.slot,
            path: pool.slot_path(record.slot),
            reused: true,
            base: None,
            warnings: Vec::new(),
        }))
}

/// Leases a slot for a branch and switches it there.
pub(crate) fn add(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
) -> Result<Added, WorktreeError> {
    check_branch(pool, &request.branch)?;
    if let Some(added) = reuse_held(pool, who, &request.branch)? {
        return Ok(added);
    }
    let warnings = refresh(pool);
    lease(pool, host, who, request, &[], warnings)
}

/// Rejects a name that git does not accept as a branch.
pub(crate) fn check_branch(pool: &Pool, branch: &str) -> Result<(), WorktreeError> {
    if branch.is_empty() || !git::succeeds(&pool.main, &["check-ref-format", "--branch", branch]) {
        return Err(WorktreeError::usage(
            "invalid_branch",
            &format!("{branch:?} is not a branch name; pick a name that git check-ref-format --branch accepts"),
        ));
    }
    Ok(())
}

/// Fetches origin before a lease; `lease` reclaims disk once it has picked a slot.
pub(crate) fn refresh(pool: &Pool) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if git::succeeds(&pool.main, &["remote", "get-url", "origin"]) {
        if let Err(error) = git::run(&pool.main, &["fetch", "--quiet", "origin"]) {
            warnings.push(Warning {
                code: "fetch_failed",
                message: format!("cannot fetch origin, using local refs: {error}"),
            });
        }
    }
    warnings
}

/// Leases a slot for a checked branch, never handing out one of the `skip` slots.
pub(crate) fn lease(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
    skip: &[u32],
    mut warnings: Vec<Warning>,
) -> Result<Added, WorktreeError> {
    let branch = request.branch.as_str();
    let lock = pool.lock()?;
    pool.forget_missing();
    let now = host.now();
    let records = pool.records();
    let mut takeover: Option<&Record> = None;
    if let Some(record) = held_on(&records, branch) {
        let path = pool.slot_path(record.slot);
        if record.held_by(who) {
            return Ok(Added {
                slot: record.slot,
                path,
                reused: true,
                base: None,
                warnings,
            });
        }
        if request.force {
            if pool.busy(record.slot) {
                return Err(busy_error(pool, record.slot));
            }
            let mut adopted = record.clone();
            adopted.lease_to(who);
            adopted.last_used = now;
            pool.write(&adopted)?;
            return Ok(Added {
                slot: record.slot,
                path,
                reused: true,
                base: None,
                warnings,
            });
        }
        let held = hold(pool, host, record);
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
            return Err(WorktreeError::refused("branch_held", &message));
        }
        takeover = Some(record);
    }
    let mut quiet: Option<std::fs::File> = None;
    if let Some(record) = takeover {
        match pool.try_quiet(record.slot)? {
            Some(guard) => quiet = Some(guard),
            None => return Err(busy_error(pool, record.slot)),
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
            ));
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
            .filter(|record| reclaimable(pool, host, record))
            .collect();
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
    let slot = chosen
        .map(|record| record.slot)
        .unwrap_or_else(|| pool.next_slot(&records));
    let fresh = chosen.is_none();
    if fresh {
        quiet = pool.try_quiet(slot)?;
    }
    let _quiet = quiet;
    let mut reclaim_warning = low_disk(host, &pool.base)
        .map(|low| reclaim_locked(pool, host, Some(slot), low, &lock).warning);
    let reuse = chosen
        .filter(|record| !record.held)
        .map(|record| record.slot);
    if let Some(dropped) = make_room(pool, host, branch, reuse)? {
        match &mut reclaim_warning {
            Some(warning) => warning.message.push_str(&format!("; {dropped}")),
            None => {
                reclaim_warning = Some(Warning {
                    code: "disk_low",
                    message: dropped,
                })
            }
        }
    }
    warnings.extend(reclaim_warning);
    let placeholder = Record {
        slot,
        held: true,
        branch: Some(branch.to_owned()),
        last_branch: chosen.and_then(|record| record.branch.clone().or(record.last_branch.clone())),
        session: who.session.clone(),
        agent: who.agent.clone(),
        pid: Some(who.pid),
        gone: false,
        unclaimed: request.unclaimed,
        broken: false,
        leased_at: now,
        last_used: now,
        lock_digest: chosen.and_then(|record| record.lock_digest.clone()),
    };
    if fresh {
        std::fs::create_dir_all(pool.slot_path(slot)).map_err(WorktreeError::io)?;
    }
    pool.write(&placeholder)?;
    let bases = scan_output_bases(&host.output_roots());
    for slot in &stale {
        // A stale slot that cannot be deleted stays recorded as broken; `add` goes on with its own slot.
        let _ = delete_slot(pool, host, *slot, &bases);
    }
    drop(lock);
    let outcome = switch_slot(pool, host, slot, fresh, reclaimed, &start, branch, &target);
    let path = pool.slot_path(slot);
    if let Err(error) = outcome {
        if fresh {
            pool.remove(slot);
            let _ = git::run(
                &pool.main,
                &["worktree", "remove", "--force", &path_text(&path)],
            );
            // Best-effort cleanup of the slot this call failed to set up.
            let _ = host.remove_tree(&path);
            let _ = git::run(&pool.main, &["worktree", "prune"]);
        } else {
            let mut freed = placeholder.clone();
            freed.free();
            let _ = pool.write(&freed);
            let _ = git::run(
                &pool.main,
                &[
                    "worktree",
                    "lock",
                    "--reason",
                    FREE_LOCK_REASON,
                    &path_text(&path),
                ],
            );
        }
        return Err(error);
    }
    let mut record = placeholder;
    record.lock_digest = git::lock_digest(&path, "HEAD");
    pool.write(&record)?;
    Ok(Added {
        slot,
        path,
        reused: false,
        base: match start {
            Start::Create(base) => Some(base),
            Start::Local | Start::Remote => None,
        },
        warnings,
    })
}

/// Refuses a lease on a full disk, first dropping the output bases of `reuse`, the free slot being reused, as a last resort.
fn make_room(
    pool: &Pool,
    host: &dyn Host,
    branch: &str,
    reuse: Option<u32>,
) -> Result<Option<String>, WorktreeError> {
    let full = |space: Option<DiskSpace>| space.filter(|space| crate::disk::is_full(space.free));
    let Some(mut space) = full(host.disk_space(&pool.base)) else {
        return Ok(None);
    };
    if let Some(slot) = reuse {
        let path = pool.slot_path(slot);
        let bases = scan_output_bases(&host.output_roots());
        let owned: Vec<&OutputBase> = bases_of(&bases, &path)
            .into_iter()
            .filter(|base| disk_low(host, &base.dir))
            .collect();
        if !owned.is_empty() && !stop_server(host, &path) {
            // The free-space reading below decides whether this helped.
            let _ = drop_bases(host, &owned);
            let after = host.disk_space(&pool.base);
            match full(after) {
                Some(still) => space = still,
                None => {
                    let free = after.map_or_else(|| "unknown".to_owned(), |after| size(after.free));
                    return Ok(Some(format!(
                            "disk full: {} free; removed the output bases of slot {slot}, which this lease reuses; {free} free now",
                            size(space.free)
                        )));
                }
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

#[allow(clippy::too_many_arguments)]
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
    let path_arg = path_text(&path);
    if fresh {
        std::fs::create_dir_all(&pool.base).map_err(WorktreeError::io)?;
        git::run(
            &pool.main,
            &["worktree", "add", "--detach", "--quiet", &path_arg, target],
        )?;
    } else {
        if reclaimed {
            stop_server(host, &path);
        }
        let _ = git::run(&pool.main, &["worktree", "unlock", &path_arg]);
        git::run(&path, &["reset", "--hard", "--quiet"])?;
        git::run(&path, &["clean", "-ffdxq", "-e", "/bazel-*"])?;
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

/// Copies the ignored files that `.worktreeinclude` names into a checkout.
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
            std::fs::create_dir_all(parent).map_err(WorktreeError::io)?;
        }
        std::fs::copy(&from, &to).map_err(WorktreeError::io)?;
        copied += 1;
    }
    Ok(copied)
}

/// Gives `who` the unclaimed lease of the slot it works in.
pub(crate) fn claim(
    pool: &Pool,
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
    pool.write(&record)?;
    Ok(Some(slot))
}

/// Frees the slot at `path`, refusing to lose work unless forced.
pub(crate) fn release(
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

/// Frees a slot only if it still holds the given lease.
pub(crate) fn release_lease(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    lease: &Record,
) -> Result<Released, WorktreeError> {
    let _lock = pool.lock()?;
    release_locked(pool, host, who, lease.slot, Some(lease), false)
}

fn release_locked(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    slot: u32,
    expected: Option<&Record>,
    force: bool,
) -> Result<Released, WorktreeError> {
    let record = pool.record(slot).ok_or_else(|| {
        WorktreeError::usage(
            "not_a_slot",
            &format!("slot {slot} has no lease record; pass a path from dx worktree list"),
        )
    })?;
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
    let slot_path = pool.slot_path(slot);
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
    if !force
        && !record.held_by(who)
        && !matches!(hold_known(pool, host, &record, false), Hold::Reclaimable)
    {
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
        if git::dirty(&slot_path)? {
            return Err(WorktreeError::refused(
                "dirty_slot",
                &format!(
                    "slot {slot} at {} has uncommitted changes; commit them or pass --force",
                    slot_path.display()
                ),
            ));
        }
        let ahead = git::unpushed(&slot_path)?;
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
    git::run(&slot_path, &["switch", "--quiet", "--detach"])?;
    let _ = git::run(
        &pool.main,
        &[
            "worktree",
            "lock",
            "--reason",
            FREE_LOCK_REASON,
            &path_text(&slot_path),
        ],
    );
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

/// Removes an external worktree and its output bases, and reports whether all of it is gone.
fn remove_external(pool: &Pool, host: &dyn Host, path: &Path, bases: &[OutputBase]) -> bool {
    let owned = bases_of(bases, path);
    stop_server(host, path);
    let arg = path_text(path);
    let _ = git::run(&pool.main, &["worktree", "unlock", &arg]);
    let _ = git::run(&pool.main, &["worktree", "remove", "--force", &arg]);
    let mut gone = host.remove_tree(path);
    let _ = git::run(&pool.main, &["worktree", "prune"]);
    for base in owned {
        gone &= host.remove_tree(&base.dir);
    }
    gone
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
    if !force {
        if git::dirty(&checkout.path)? {
            return Err(WorktreeError::refused(
                "dirty_slot",
                &format!(
                    "worktree {} has uncommitted changes; commit them or pass --force",
                    checkout.path.display()
                ),
            ));
        }
        let ahead = git::unpushed(&checkout.path)?;
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
    let _lock = pool.lock()?;
    let bases = scan_output_bases(&host.output_roots());
    let _ = remove_external(pool, host, &checkout.path, &bases);
    Ok(Released {
        slot: None,
        path: checkout.path,
        branch: checkout.branch,
        already_free: false,
        removed: true,
        warnings: Vec::new(),
    })
}

/// Why `prune` keeps an external checkout, or `None` when it may remove it.
///
/// A branch checkout is removable when its branch tracks `origin/<branch>` and is merged into `base` or its
/// upstream is gone. A detached checkout is removable when HEAD is in `base` and git has not touched it for
/// `EXTERNAL_GRACE_SECS`. Any git or file error keeps the checkout.
fn external_plan_reason(
    pool: &Pool,
    host: &dyn Host,
    checkout: &git::Checkout,
    base: Option<&str>,
    servers: &[Server],
) -> Option<&'static str> {
    if let Some(reason) = external_keep_reason(checkout, servers) {
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

/// Whether git touched the checkout within `EXTERNAL_GRACE_SECS`, or its times cannot be read.
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

fn external_keep_reason(checkout: &git::Checkout, servers: &[Server]) -> Option<&'static str> {
    let path = &checkout.path;
    if checkout.locked {
        Some("locked")
    } else if server_in(servers, path) {
        Some("server")
    } else if !matches!(git::dirty(path), Ok(false)) {
        Some("dirty")
    } else if !matches!(git::unpushed(path), Ok(0)) {
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

/// Stops the Bazel server of path and reports whether one still runs.
fn stop_server(host: &dyn Host, path: &Path) -> bool {
    if !server_in(&host.servers(), path) {
        return false;
    }
    host.bazel_shutdown(path);
    server_in(&host.servers(), path)
}

/// Lists every slot and hand-made checkout, with disk use when asked.
pub(crate) fn list(pool: &Pool, host: &dyn Host, disk: bool) -> Result<Listing, WorktreeError> {
    pool.forget_missing();
    let servers = host.servers();
    let bases = if disk {
        scan_output_bases(&host.output_roots())
    } else {
        Vec::new()
    };
    let mut rows = Vec::new();
    let mut slot_disk = 0;
    for record in pool.records() {
        let path = pool.slot_path(record.slot);
        let held = hold(pool, host, &record);
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
                _ => None,
            },
            server: server_in(&servers, &path),
            disk_bytes,
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
            disk_bytes,
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

/// The size of the disk cache Bazel uses for `build`; 0 when it is off or missing.
pub(crate) fn disk_cache_bytes(host: &dyn Host) -> Result<u64, WorktreeError> {
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

/// The checkout that holds a branch.
pub(crate) fn path_of(pool: &Pool, branch: &str) -> Result<PathBuf, WorktreeError> {
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

/// Plans `prune` and, with `apply`, deletes the plan under the pool lock.
pub(crate) fn prune(
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
        used_downloads,
    } = prune_apply(pool, host, &plan, &lock)?;
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
    })
}

/// What makes a free slot's output base a candidate for deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// The base's disk is low.
    LowDisk,
    /// The caller asked for it, whatever the disk state.
    OnDemand,
}

/// Free slots' output bases that `trigger` selects, least recently used slot first.
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

/// Finds idle free slots, removable external worktrees (and the reason it keeps each other one), orphaned
/// output bases, free slots' output bases when `trigger` selects them, and stale repository cache downloads.
pub(crate) fn prune_plan(
    pool: &Pool,
    host: &dyn Host,
    trigger: Trigger,
) -> Result<PrunePlan, WorktreeError> {
    pool.forget_missing();
    let now = host.now();
    let servers = host.servers();
    let roots = host.output_roots();
    let bases = scan_output_bases(&roots);
    let nested_root = host.nested_root().map(|root| resolved(&root));
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
        if base.workspace.exists() || under(nested_root.as_deref(), &base.dir) {
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
    plan_nested(host, &servers, trigger, now, &mut plan)?;
    Ok(plan)
}

/// One test case of a nested-Bazel slot: its output base under `u` and the workspace it was made for.
struct NestedCase {
    slot: PathBuf,
    base: OutputBase,
    /// When the case's workspace last changed, or `None` when the workspace is gone.
    used: Option<u64>,
}

impl NestedCase {
    fn owner(&self) -> BaseOwner {
        BaseOwner::Nested {
            slot: self.slot.clone(),
            case: self
                .base
                .workspace
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// The case's workspace when it lies in its slot's `w`.
    fn own_workspace(&self) -> Option<&Path> {
        let workspaces = resolved(&self.slot.join(nested_bazel::WORKSPACES));
        let workspace = resolved(&self.base.workspace);
        (workspace != workspaces && workspace.starts_with(&workspaces))
            .then_some(self.base.workspace.as_path())
    }

    fn in_use(&self, servers: &[Server]) -> bool {
        base_in_use(servers, &self.base) || server_in(servers, &self.base.workspace)
    }
}

/// Whether `path` lies under `root`.
fn under(root: Option<&Path>, path: &Path) -> bool {
    root.is_some_and(|root| resolved(path).starts_with(root))
}

/// The test cases of every nested-Bazel slot, least recently used first.
fn nested_cases(host: &dyn Host) -> Vec<NestedCase> {
    let Some(root) = host.nested_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut slots: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().parse::<u32>().is_ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    slots.sort();
    let mut cases = Vec::new();
    for slot in slots {
        for base in scan_output_bases(&[slot.join(nested_bazel::USER_ROOT)]) {
            let used = std::fs::metadata(&base.workspace)
                .and_then(|meta| meta.modified())
                .ok()
                .map(|modified| {
                    modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|elapsed| elapsed.as_secs())
                        .unwrap_or(0)
                });
            cases.push(NestedCase {
                slot: slot.clone(),
                base,
                used,
            });
        }
    }
    cases.sort_by(|left, right| (left.used, &left.base.dir).cmp(&(right.used, &right.base.dir)));
    cases
}

/// Locks a nested-Bazel slot without waiting or creating the file; `None` when busy or missing.
fn try_nested_lock(slot: &Path) -> Option<std::fs::File> {
    let file = std::fs::File::options()
        .write(true)
        .open(slot.join(nested_bazel::LOCK))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// Whether a nested-Bazel slot is free right now; the probe lock is released before this returns.
fn nested_slot_free(slot: &Path) -> bool {
    try_nested_lock(slot).is_some()
}

/// Lists the test cases of free nested-Bazel slots: those idle for 30 days, and with `--bases` or on a low
/// disk every one, least recently used first.
fn plan_nested(
    host: &dyn Host,
    servers: &[Server],
    trigger: Trigger,
    now: u64,
    plan: &mut PrunePlan,
) -> Result<(), WorktreeError> {
    let mut free: Vec<(PathBuf, bool)> = Vec::new();
    for case in nested_cases(host) {
        stop_check(host)?;
        let is_free = match free.iter().find(|(slot, _)| *slot == case.slot) {
            Some((_, is_free)) => *is_free,
            None => {
                let is_free = nested_slot_free(&case.slot);
                free.push((case.slot.clone(), is_free));
                is_free
            }
        };
        if !is_free {
            continue;
        }
        let stale = case
            .used
            .is_none_or(|used| now.saturating_sub(used) >= STALE_AFTER_SECS);
        if !(stale || trigger == Trigger::OnDemand || disk_low(host, &case.base.dir)) {
            continue;
        }
        if case.in_use(servers) {
            plan.kept_bases.push(PruneKeptBase {
                dir: case.base.dir.clone(),
                workspace: case.base.workspace.clone(),
                owner: case.owner(),
                reason: "server",
            });
            continue;
        }
        let mut bytes = measure(host, &case.base.dir)?;
        if let Some(workspace) = case.own_workspace().filter(|path| path.exists()) {
            bytes += measure(host, workspace)?;
        }
        plan.bases.push(PruneBase {
            dir: case.base.dir.clone(),
            workspace: case.base.workspace.clone(),
            bytes,
            owner: case.owner(),
        });
    }
    Ok(())
}

/// Deletes one nested-Bazel test case: its output base and its workspace in the slot.
fn drop_nested(host: &dyn Host, case: &NestedCase) -> bool {
    let mut gone = host.remove_tree(&case.base.dir);
    if let Some(workspace) = case.own_workspace() {
        gone &= host.remove_tree(workspace);
    }
    gone
}

/// The nested-Bazel slot locks one apply or reclaim holds, taken once per slot.
#[derive(Default)]
struct NestedLocks {
    held: Vec<(PathBuf, Option<std::fs::File>)>,
}

impl NestedLocks {
    /// Whether this holds the slot's lock, taking it on first use.
    fn hold(&mut self, slot: &Path) -> bool {
        if let Some((_, lock)) = self.held.iter().find(|(held, _)| held == slot) {
            return lock.is_some();
        }
        let lock = try_nested_lock(slot);
        let taken = lock.is_some();
        self.held.push((slot.to_path_buf(), lock));
        taken
    }
}

fn base_in_use(servers: &[Server], base: &OutputBase) -> bool {
    servers.iter().any(|server| {
        server
            .output_base
            .as_deref()
            .is_some_and(|dir| same_path(dir, &base.dir))
    })
}

/// Deletes what `prune_plan` found while the caller holds the pool lock, and returns what it kept.
pub(crate) fn prune_apply(
    pool: &Pool,
    host: &dyn Host,
    plan: &PrunePlan,
    _lock: &dx_atomic_fs::lease::ExclusiveLease,
) -> Result<Applied, WorktreeError> {
    let bases = scan_output_bases(&host.output_roots());
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
            delete_slot(pool, host, slot.slot, &bases)
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
        if let Some(reason) = external_keep_reason(&checkout, &servers) {
            kept.push(PruneKept {
                path: checkout.path,
                branch: checkout.branch,
                reason,
            });
            continue;
        }
        stop_check(host)?;
        if !remove_external(pool, host, &external.path, &bases) {
            kept.push(PruneKept {
                path: checkout.path,
                branch: checkout.branch,
                reason: "busy",
            });
        }
    }
    let mut nested = NestedLocks::default();
    for base in &plan.bases {
        stop_check(host)?;
        let skip = |reason| PruneKeptBase {
            dir: base.dir.clone(),
            workspace: base.workspace.clone(),
            owner: base.owner.clone(),
            reason,
        };
        if let BaseOwner::Nested { slot, .. } = &base.owner {
            if !nested.hold(slot) {
                skipped.push(skip("busy"));
                continue;
            }
            let case = NestedCase {
                slot: slot.clone(),
                base: OutputBase {
                    dir: base.dir.clone(),
                    workspace: base.workspace.clone(),
                },
                used: None,
            };
            if case.in_use(&host.servers()) {
                skipped.push(skip("server"));
                continue;
            }
            if !drop_nested(host, &case) {
                skipped.push(skip("busy"));
            }
            continue;
        }
        if let Some(slot) = base.owner.slot() {
            if !pool.record(slot).is_some_and(|record| !record.held) {
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
        used_downloads,
    })
}

/// The downloads under `roots` that no build used for 30 days, grouped by repository cache.
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

/// Deletes one download if no build used it since it went stale, checking right before the delete.
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
    crate::disk::is_low(space.free, space.total)
}

/// Whether the disk holding `path` has under 20 GiB and under 10% free, or under 1 GiB.
pub(crate) fn disk_low(host: &dyn Host, path: &Path) -> bool {
    host.disk_space(path).is_some_and(is_low)
}

/// The low disk holding `base`, or else the low Bazel output root or nested-Bazel root with the least free
/// space.
pub(crate) fn low_disk(host: &dyn Host, base: &Path) -> Option<(PathBuf, DiskSpace)> {
    low_disk_reading(host, base).1
}

/// The reading of the disk holding `base`, and the low disk `low_disk` finds from it.
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
    let nested = host.nested_root().filter(|root| root.is_dir());
    let low = host
        .output_roots()
        .into_iter()
        .chain(nested)
        .filter_map(|root| {
            host.disk_space(&root)
                .filter(|space| is_low(*space))
                .map(|space| (root, space))
        })
        .min_by_key(|(_, space)| space.free);
    (Some(space), low)
}

/// Deletes the output bases and reports whether all of them are gone.
fn drop_bases(host: &dyn Host, owned: &[&OutputBase]) -> bool {
    let mut gone = true;
    for base in owned {
        gone &= host.remove_tree(&base.dir);
    }
    gone
}

/// Reclaims output bases on low disk, reporting a failure as a disk_low warning.
pub(crate) fn reclaim(pool: &Pool, host: &dyn Host, keep: Option<u32>) -> Option<Reclaim> {
    match reclaim_if_low(pool, host, keep) {
        Ok(reclaimed) => reclaimed,
        Err(error) => Some(Reclaim {
            warning: reclaim_failed(&error),
            removed: false,
        }),
    }
}

/// The disk_low warning for a reclaim that could not run.
pub(crate) fn reclaim_failed(error: &WorktreeError) -> Warning {
    Warning {
        code: "disk_low",
        message: format!("low disk, cannot reclaim output bases: {error}"),
    }
}

/// Drops orphaned output bases, then repository cache downloads no build used for 30 days, then the test
/// cases of free nested-Bazel slots, then free slots' output bases, each oldest first, while the disk each one
/// is on is low.
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

/// Reclaims on the low disk `low` for a caller that holds the pool lock, never touching the `keep` slot.
fn reclaim_locked(
    pool: &Pool,
    host: &dyn Host,
    keep: Option<u32>,
    (disk, before): (PathBuf, DiskSpace),
    _lock: &dx_atomic_fs::lease::ExclusiveLease,
) -> Reclaim {
    let roots = host.output_roots();
    let bases = scan_output_bases(&roots);
    let servers = host.servers();
    let nested_root = host.nested_root().map(|root| resolved(&root));
    let mut orphans = 0usize;
    let mut slots: Vec<u32> = Vec::new();
    for base in bases.iter().filter(|base| {
        !base.workspace.exists()
            && !under(nested_root.as_deref(), &base.dir)
            && !base_in_use(&servers, base)
    }) {
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
    let mut nested = 0usize;
    let mut locks = NestedLocks::default();
    for case in nested_cases(host) {
        if host.interrupted().is_some() || !disk_low(host, &case.base.dir) {
            continue;
        }
        if !locks.hold(&case.slot) || case.in_use(&host.servers()) {
            continue;
        }
        if drop_nested(host, &case) {
            nested += 1;
        }
    }
    drop(locks);
    let picked = free_slot_bases(pool, host, &bases, Trigger::LowDisk, |record| {
        Some(record.slot) == keep || pool.busy(record.slot)
    });
    for (slot, owned) in picked {
        let path = pool.slot_path(slot);
        if stop_server(host, &path) {
            continue;
        }
        if drop_bases(host, &owned) {
            slots.push(slot);
        }
    }
    let after = host.disk_space(&disk).unwrap_or(before);
    let mut message = format!("low disk: {} free", size(before.free));
    let mut removed = Vec::new();
    if orphans > 0 {
        removed.push(format!("{orphans} orphaned output base(s)"));
    }
    if downloads > 0 {
        removed.push(format!("{downloads} repository cache download(s)"));
    }
    if nested > 0 {
        removed.push(format!("{nested} nested Bazel output base(s)"));
    }
    if !slots.is_empty() {
        let numbers: Vec<String> = slots.iter().map(u32::to_string).collect();
        removed.push(format!(
            "the output bases of free slot(s) {}",
            numbers.join(", ")
        ));
    }
    let reclaimed = !removed.is_empty();
    if !reclaimed {
        message.push_str("; nothing to reclaim");
    } else {
        message.push_str(&format!(
            "; removed {}; {} free now",
            removed.join(" and "),
            size(after.free)
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
    }
}

/// A byte count in binary units.
pub(crate) fn size(bytes: u64) -> String {
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
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// What `delete_slot` could not delete.
#[derive(Debug, Default)]
struct Leftover {
    /// Why the slot is still there, recorded as broken or untouched.
    slot: Option<&'static str>,
    /// Output bases left behind by a slot that is gone.
    bases: Vec<OutputBase>,
}

/// Deletes a slot and its output bases, and returns what is still on disk.
fn delete_slot(pool: &Pool, host: &dyn Host, slot: u32, bases: &[OutputBase]) -> Leftover {
    let Ok(Some(_quiet)) = pool.try_quiet(slot) else {
        return Leftover {
            slot: Some("busy"),
            bases: Vec::new(),
        };
    };
    let path = pool.slot_path(slot);
    let owned = bases_of(bases, &path);
    stop_server(host, &path);
    let arg = path_text(&path);
    let _ = git::run(&pool.main, &["worktree", "unlock", &arg]);
    let _ = git::run(&pool.main, &["worktree", "remove", "--force", &arg]);
    let gone = host.remove_tree(&path);
    let _ = git::run(&pool.main, &["worktree", "prune"]);
    let stuck: Vec<OutputBase> = owned
        .into_iter()
        .filter(|base| !host.remove_tree(&base.dir))
        .cloned()
        .collect();
    if gone {
        pool.remove(slot);
        return Leftover {
            slot: None,
            bases: stuck,
        };
    }
    mark_broken(pool, slot);
    Leftover {
        slot: Some("busy"),
        bases: Vec::new(),
    }
}

/// Keeps a half-deleted slot recorded so `prune` retries it, but out of `add`'s reach.
fn mark_broken(pool: &Pool, slot: u32) {
    let Some(mut record) = pool.record(slot) else {
        return;
    };
    record.broken = true;
    if pool.write(&record).is_err() {
        // A free record would hand the partial tree out again; `next_slot` skips the leftover dir.
        pool.remove(slot);
    }
}
