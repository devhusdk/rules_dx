use std::path::{Path, PathBuf};

use super::git;
use super::host::{same_path, scan_output_bases, DiskSpace, Host, OutputBase, Server};
use super::pool::{Pool, Record, Who};
use super::WorktreeError;

const STALE_AFTER_SECS: u64 = 30 * 24 * 60 * 60;
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

/// One output base whose workspace is gone, or of a free slot when its disk is low.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) bytes: u64,
    pub(crate) slot: Option<u32>,
}

/// One output base `prune` keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneKeptBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) slot: Option<u32>,
    pub(crate) reason: &'static str,
}

/// One external worktree `prune` would remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneExternal {
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) bytes: u64,
}

/// One external worktree `prune --apply` kept because it changed after the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneKept {
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) reason: &'static str,
}

/// What `prune` deleted or would delete, and what it kept.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Pruned {
    pub(crate) plan: PrunePlan,
    pub(crate) kept: Vec<PruneKept>,
    pub(crate) skipped_bases: Vec<PruneKeptBase>,
}

/// What `prune` found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PrunePlan {
    pub(crate) slots: Vec<PruneSlot>,
    pub(crate) externals: Vec<PruneExternal>,
    pub(crate) bases: Vec<PruneBase>,
    pub(crate) kept_bases: Vec<PruneKeptBase>,
}

impl PrunePlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.slots.is_empty() && self.externals.is_empty() && self.bases.is_empty()
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.slots.iter().map(|slot| slot.bytes).sum::<u64>()
            + self
                .externals
                .iter()
                .map(|external| external.bytes)
                .sum::<u64>()
            + self.bases.iter().map(|base| base.bytes).sum::<u64>()
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

fn rank(candidates: &mut Vec<&Record>, branch: &str, digest: Option<&str>, oldest_first: bool) {
    if oldest_first {
        candidates.sort_by_key(|record| (record.last_used, record.slot));
        return;
    }
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

/// Leases a slot for a branch and switches it there.
pub(crate) fn add(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
) -> Result<Added, WorktreeError> {
    check_branch(pool, &request.branch)?;
    let warnings = refresh(pool, host);
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

/// Reclaims a dead slot and fetches origin before a lease.
pub(crate) fn refresh(pool: &Pool, host: &dyn Host) -> Vec<Warning> {
    let mut warnings: Vec<Warning> = reclaim(pool, host, None)
        .map(|reclaimed| reclaimed.warning)
        .into_iter()
        .collect();
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
    warnings: Vec<Warning>,
) -> Result<Added, WorktreeError> {
    let branch = request.branch.as_str();
    let lock = pool.lock()?;
    pool.forget_missing();
    let now = host.now();
    let records = pool.records();
    let mut takeover: Option<&Record> = None;
    if let Some(record) = records
        .iter()
        .find(|record| record.held && record.branch.as_deref() == Some(branch))
    {
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
    let (base_space, low) = low_disk_reading(host, &pool.base);
    let disk_is_low = low.is_some();
    if let Some(space) = base_space.filter(|space| crate::disk::is_full(space.free)) {
        return Err(WorktreeError::refused(
            "disk_full",
            &format!(
                "disk full: {} free at {}; cannot check out {branch}; free disk space with `dx worktree prune --apply --bases`",
                size(space.free),
                pool.base.display()
            ),
        ));
    }
    let mut free: Vec<&Record> = records
        .iter()
        .filter(|record| !record.held && !skip.contains(&record.slot))
        .collect();
    rank(&mut free, branch, digest.as_deref(), disk_is_low);
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
        rank(&mut candidates, branch, digest.as_deref(), disk_is_low);
        chosen = first_quiet(pool, &candidates, &mut quiet)?;
        reclaimed = chosen.is_some();
    }
    let stale: Vec<u32> = records
        .iter()
        .filter(|record| {
            !record.held
                && Some(record.slot) != chosen.map(|picked| picked.slot)
                && now.saturating_sub(record.last_used) >= STALE_AFTER_SECS
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
        delete_slot(pool, host, *slot, &bases);
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
            remove_tree(&path);
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
            host.bazel_shutdown(&path);
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
    host.bazel_shutdown(&slot_path);
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

fn remove_external(pool: &Pool, host: &dyn Host, path: &Path, bases: &[OutputBase]) {
    let owned = bases_of(bases, path);
    host.bazel_shutdown(path);
    let arg = path_text(path);
    let _ = git::run(&pool.main, &["worktree", "unlock", &arg]);
    let _ = git::run(&pool.main, &["worktree", "remove", "--force", &arg]);
    remove_tree(path);
    let _ = git::run(&pool.main, &["worktree", "prune"]);
    for base in owned {
        remove_tree(&base.dir);
    }
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
    remove_external(pool, host, &checkout.path, &bases);
    Ok(Released {
        slot: None,
        path: checkout.path,
        branch: checkout.branch,
        already_free: false,
        removed: true,
        warnings: Vec::new(),
    })
}

fn external_prunable(
    pool: &Pool,
    checkout: &git::Checkout,
    base: Option<&str>,
    servers: &[Server],
) -> bool {
    if external_keep_reason(checkout, servers).is_some() {
        return false;
    }
    let path = &checkout.path;
    let branch = checkout.branch.as_deref();
    let Some(branch) = branch.filter(|branch| git::tracks_own_remote(&pool.main, branch)) else {
        return false;
    };
    let merged = base.is_some_and(|base| git::is_ancestor(path, "HEAD", base));
    merged || git::upstream_gone(&pool.main, branch)
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
        Some(match host.disk_cache().filter(|dir| dir.is_dir()) {
            Some(dir) => measure(host, &dir)?,
            None => 0,
        })
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
    let (kept, skipped_bases) = prune_apply(pool, host, &plan, &lock)?;
    plan.externals
        .retain(|external| !kept.iter().any(|item| item.path == external.path));
    plan.bases
        .retain(|base| !skipped_bases.iter().any(|item| item.dir == base.dir));
    Ok(Pruned {
        plan,
        kept,
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

/// Finds free slots idle for 30 days, orphaned output bases, and free slots' output bases when `trigger` selects them.
pub(crate) fn prune_plan(
    pool: &Pool,
    host: &dyn Host,
    trigger: Trigger,
) -> Result<PrunePlan, WorktreeError> {
    pool.forget_missing();
    let now = host.now();
    let servers = host.servers();
    let bases = scan_output_bases(&host.output_roots());
    let mut plan = PrunePlan::default();
    for record in pool.records() {
        let idle = now.saturating_sub(record.last_used);
        if record.held || idle < STALE_AFTER_SECS {
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
        if same_path(&checkout.path, &pool.main)
            || pool.slot_of(&checkout.path).is_some()
            || !external_prunable(pool, &checkout, default.as_deref(), &servers)
        {
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
                slot: None,
                reason: "server",
            });
            continue;
        }
        plan.bases.push(PruneBase {
            dir: base.dir.clone(),
            workspace: base.workspace.clone(),
            bytes: measure(host, &base.dir)?,
            slot: None,
        });
    }
    let listed = free_slot_bases(pool, host, &bases, trigger, |record| {
        plan.slots.iter().any(|slot| slot.slot == record.slot)
    });
    for (slot, owned) in listed {
        for base in owned {
            plan.bases.push(PruneBase {
                dir: base.dir.clone(),
                workspace: base.workspace.clone(),
                bytes: measure(host, &base.dir)?,
                slot: Some(slot),
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

/// Deletes what `prune_plan` found while the caller holds the pool lock, and returns what it kept.
pub(crate) fn prune_apply(
    pool: &Pool,
    host: &dyn Host,
    plan: &PrunePlan,
    _lock: &dx_atomic_fs::lease::ExclusiveLease,
) -> Result<(Vec<PruneKept>, Vec<PruneKeptBase>), WorktreeError> {
    let bases = scan_output_bases(&host.output_roots());
    for slot in &plan.slots {
        stop_check(host)?;
        if pool.record(slot.slot).is_some_and(|record| !record.held) {
            delete_slot(pool, host, slot.slot, &bases);
        }
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
        remove_external(pool, host, &external.path, &bases);
    }
    let mut skipped = Vec::new();
    for base in &plan.bases {
        stop_check(host)?;
        if let Some(slot) = base.slot {
            let skip = |reason| PruneKeptBase {
                dir: base.dir.clone(),
                workspace: base.workspace.clone(),
                slot: Some(slot),
                reason,
            };
            if !pool.record(slot).is_some_and(|record| !record.held) {
                skipped.push(skip("held"));
                continue;
            }
            if pool.busy(slot) {
                skipped.push(skip("busy"));
                continue;
            }
            let path = pool.slot_path(slot);
            host.bazel_shutdown(&path);
            if server_in(&host.servers(), &path) {
                skipped.push(skip("server"));
                continue;
            }
        }
        remove_tree(&base.dir);
    }
    Ok((kept, skipped))
}

fn is_low(space: DiskSpace) -> bool {
    crate::disk::is_low(space.free, space.total)
}

/// Whether the disk holding `path` has under 20 GiB and under 10% free.
pub(crate) fn disk_low(host: &dyn Host, path: &Path) -> bool {
    host.disk_space(path).is_some_and(is_low)
}

/// The low disk holding `base`, or else the low Bazel output root with the least free space.
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
    let low = host
        .output_roots()
        .into_iter()
        .filter_map(|root| {
            host.disk_space(&root)
                .filter(|space| is_low(*space))
                .map(|space| (root, space))
        })
        .min_by_key(|(_, space)| space.free);
    (Some(space), low)
}

fn drop_bases(owned: &[&OutputBase]) {
    for base in owned {
        remove_tree(&base.dir);
    }
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

/// Drops orphaned output bases, then free slots' output bases oldest first, while each base's disk is low.
pub(crate) fn reclaim_if_low(
    pool: &Pool,
    host: &dyn Host,
    keep: Option<u32>,
) -> Result<Option<Reclaim>, WorktreeError> {
    let Some((disk, before)) = low_disk(host, &pool.base) else {
        return Ok(None);
    };
    let _lock = pool.lock()?;
    pool.forget_missing();
    let bases = scan_output_bases(&host.output_roots());
    let servers = host.servers();
    let mut orphans = 0usize;
    let mut slots: Vec<u32> = Vec::new();
    for base in bases
        .iter()
        .filter(|base| !base.workspace.exists() && !base_in_use(&servers, base))
    {
        if !disk_low(host, &base.dir) {
            continue;
        }
        drop_bases(&[base]);
        orphans += 1;
    }
    let picked = free_slot_bases(pool, host, &bases, Trigger::LowDisk, |record| {
        Some(record.slot) == keep || pool.busy(record.slot)
    });
    for (slot, owned) in picked {
        let path = pool.slot_path(slot);
        host.bazel_shutdown(&path);
        if server_in(&host.servers(), &path) {
            continue;
        }
        drop_bases(&owned);
        slots.push(slot);
    }
    let after = host.disk_space(&disk).unwrap_or(before);
    let mut message = format!("low disk: {} free", size(before.free));
    let mut removed = Vec::new();
    if orphans > 0 {
        removed.push(format!("{orphans} orphaned output base(s)"));
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
    Ok(Some(Reclaim {
        warning: Warning {
            code: "disk_low",
            message,
        },
        removed: reclaimed,
    }))
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

fn delete_slot(pool: &Pool, host: &dyn Host, slot: u32, bases: &[OutputBase]) {
    let Ok(Some(_quiet)) = pool.try_quiet(slot) else {
        return;
    };
    let path = pool.slot_path(slot);
    let owned = bases_of(bases, &path);
    host.bazel_shutdown(&path);
    let arg = path_text(&path);
    let _ = git::run(&pool.main, &["worktree", "unlock", &arg]);
    let _ = git::run(&pool.main, &["worktree", "remove", "--force", &arg]);
    remove_tree(&path);
    let _ = git::run(&pool.main, &["worktree", "prune"]);
    pool.remove(slot);
    for base in owned {
        remove_tree(&base.dir);
    }
}

fn make_writable(path: &Path) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        permissions.set_mode(permissions.mode() | 0o700);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
    }
    let _ = std::fs::set_permissions(path, permissions);
    if metadata.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                make_writable(&entry.path());
            }
        }
    }
}

/// Removes a directory tree, including read-only entries.
pub(crate) fn remove_tree(path: &Path) {
    if std::fs::remove_dir_all(path).is_err() {
        make_writable(path);
        let _ = std::fs::remove_dir_all(path);
    }
}
