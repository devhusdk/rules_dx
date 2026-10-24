use std::path::{Path, PathBuf};

use super::git;
use super::host::{same_path, scan_output_bases, Host, OutputBase, Server};
use super::pool::{Pool, Record, Who};
use super::WorktreeError;

const STALE_AFTER_SECS: u64 = 30 * 24 * 60 * 60;
const LOW_DISK_PERCENT: f64 = 10.0;
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
}

/// The slot an `add` handed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Added {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) reused: bool,
    pub(crate) warnings: Vec<Warning>,
}

/// A problem `add` worked around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Warning {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

/// The slot a `release` freed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Released {
    pub(crate) slot: Option<u32>,
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) already_free: bool,
    pub(crate) removed: bool,
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

/// One output base whose workspace is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PruneBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) bytes: u64,
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

/// What `prune` found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PrunePlan {
    pub(crate) slots: Vec<PruneSlot>,
    pub(crate) externals: Vec<PruneExternal>,
    pub(crate) bases: Vec<PruneBase>,
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

fn reclaimable(pool: &Pool, host: &dyn Host, record: &Record) -> bool {
    if !record.held || holder_alive(host, record) {
        return false;
    }
    let path = pool.slot_path(record.slot);
    matches!(git::dirty(&path), Ok(false)) && matches!(git::unpushed(&path), Ok(0))
}

fn waiting_reason(pool: &Pool, host: &dyn Host, record: &Record) -> Option<&'static str> {
    if !record.held || holder_alive(host, record) {
        return None;
    }
    let path = pool.slot_path(record.slot);
    if !matches!(git::dirty(&path), Ok(false)) {
        return Some("holder gone, tree has uncommitted changes");
    }
    if !matches!(git::unpushed(&path), Ok(0)) {
        return Some("holder gone, commits are not pushed");
    }
    None
}

fn describe_holder(host: &dyn Host, record: &Record) -> String {
    record
        .holder(holder_alive(host, record))
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
                &format!("--create base {requested} does not name a commit"),
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

/// Leases a slot for a branch and switches it there.
pub(crate) fn add(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
) -> Result<Added, WorktreeError> {
    add_except(pool, host, who, request, &[])
}

/// Leases a slot like `add`, but never hands out one of the `skip` slots.
pub(crate) fn add_except(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    request: &AddRequest,
    skip: &[u32],
) -> Result<Added, WorktreeError> {
    let branch = request.branch.as_str();
    if branch.is_empty() || !git::succeeds(&pool.main, &["check-ref-format", "--branch", branch]) {
        return Err(WorktreeError::usage(
            "invalid_branch",
            &format!("{branch:?} is not a branch name"),
        ));
    }
    let mut warnings = Vec::new();
    if git::succeeds(&pool.main, &["remote", "get-url", "origin"]) {
        if let Err(error) = git::run(&pool.main, &["fetch", "--quiet", "origin"]) {
            warnings.push(Warning {
                code: "fetch_failed",
                message: format!("cannot fetch origin, using local refs: {error}"),
            });
        }
    }
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
                warnings,
            });
        }
        if request.force {
            let mut adopted = record.clone();
            adopted.lease_to(who);
            adopted.last_used = now;
            pool.write(&adopted)?;
            return Ok(Added {
                slot: record.slot,
                path,
                reused: true,
                warnings,
            });
        }
        if !reclaimable(pool, host, record) {
            let mut message = format!(
                "branch {branch} is held by slot {} at {}, leased to {}",
                record.slot,
                path.display(),
                describe_holder(host, record)
            );
            if let Some(reason) = waiting_reason(pool, host, record) {
                message.push_str(&format!("; {reason}"));
            }
            message.push_str("; pass --force to take it over");
            return Err(WorktreeError::refused("branch_held", &message));
        }
        takeover = Some(record);
    }
    for checkout in git::checkouts(&pool.main)? {
        if checkout.branch.as_deref() == Some(branch)
            && !takeover
                .is_some_and(|record| same_path(&checkout.path, &pool.slot_path(record.slot)))
        {
            return Err(WorktreeError::refused(
                "branch_held",
                &format!(
                    "branch {branch} is checked out at {}",
                    checkout.path.display()
                ),
            ));
        }
    }
    let start = resolve_start(pool, branch, request.create.as_deref())?;
    let target = target_ref(&start, branch);
    let digest = git::lock_digest(&pool.main, &target);
    let low_disk = host
        .free_disk_percent(&pool.base)
        .is_some_and(|percent| percent < LOW_DISK_PERCENT);
    let mut free: Vec<&Record> = records
        .iter()
        .filter(|record| !record.held && !skip.contains(&record.slot))
        .collect();
    rank(&mut free, branch, digest.as_deref(), low_disk);
    let mut reclaimed = takeover.is_some();
    let mut chosen: Option<&Record> = takeover.or_else(|| free.first().copied());
    if chosen.is_none() {
        let mut candidates: Vec<&Record> = records
            .iter()
            .filter(|record| reclaimable(pool, host, record))
            .collect();
        rank(&mut candidates, branch, digest.as_deref(), low_disk);
        chosen = candidates.first().copied();
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
    let placeholder = Record {
        slot,
        held: true,
        branch: Some(branch.to_owned()),
        last_branch: chosen.and_then(|record| record.branch.clone().or(record.last_branch.clone())),
        session: who.session.clone(),
        agent: who.agent.clone(),
        pid: Some(who.pid),
        gone: false,
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

/// Frees the slot at `path`, refusing to lose work unless forced.
pub(crate) fn release(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    path: &Path,
    force: bool,
) -> Result<Released, WorktreeError> {
    let Some(slot) = pool.slot_of(path) else {
        return release_external(pool, host, path, force);
    };
    let _lock = pool.lock()?;
    release_locked(pool, host, who, slot, None, force)
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
        WorktreeError::usage("not_a_slot", &format!("slot {slot} has no lease record"))
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
            &format!("slot {slot} was leased again; leaving it"),
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
        });
    }
    if !force && !record.held_by(who) && !reclaimable(pool, host, &record) {
        return Err(WorktreeError::usage(
            "slot_leased",
            &format!(
                "slot {slot} is leased to {}; only that holder can release it unless it is reclaimable, or pass --force",
                describe_holder(host, &record)
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
    })
}

fn external_checkout(pool: &Pool, path: &Path) -> Option<git::Checkout> {
    git::checkouts(&pool.main)
        .ok()?
        .into_iter()
        .find(|checkout| {
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
                "{} is not a dx worktree slot or a worktree of this repository",
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
    let merged = base.is_some_and(|base| git::is_ancestor(path, "HEAD", base))
        && branch.is_some_and(|branch| git::has_upstream(&pool.main, branch));
    let deleted = branch.is_some_and(|branch| git::upstream_gone(&pool.main, branch));
    merged || deleted
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
        let state = if !record.held {
            State::Free
        } else if reclaimable(pool, host, &record) {
            State::Reclaimable
        } else {
            State::Held
        };
        let disk_bytes = disk.then(|| {
            host.disk_bytes(&path)
                + bases
                    .iter()
                    .filter(|base| same_path(&base.workspace, &path))
                    .map(|base| host.disk_bytes(&base.dir))
                    .sum::<u64>()
        });
        slot_disk += disk_bytes.unwrap_or(0);
        rows.push(Row {
            slot: Some(record.slot),
            branch: git::current_branch(&path).or(record.branch.clone()),
            holder: record.holder(holder_alive(host, &record)),
            state,
            reason: waiting_reason(pool, host, &record),
            server: server_in(&servers, &path),
            disk_bytes,
            path,
        });
    }
    for checkout in git::checkouts(&pool.main)? {
        if same_path(&checkout.path, &pool.main) || pool.slot_of(&checkout.path).is_some() {
            continue;
        }
        rows.push(Row {
            slot: None,
            branch: checkout.branch.clone(),
            holder: None,
            state: State::External,
            reason: None,
            server: server_in(&servers, &checkout.path),
            disk_bytes: disk.then(|| host.disk_bytes(&checkout.path)),
            path: checkout.path,
        });
    }
    Ok(Listing {
        rows,
        servers: servers.len() as u64,
        servers_rss_bytes: servers.iter().map(|server| server.rss_bytes).sum(),
        disk_bytes: disk.then_some(slot_disk),
        disk_cache_bytes: disk.then(|| {
            host.disk_cache()
                .filter(|dir| dir.is_dir())
                .map_or(0, |dir| host.disk_bytes(&dir))
        }),
    })
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
    for checkout in git::checkouts(&pool.main)? {
        if checkout.branch.as_deref() == Some(branch) {
            return Ok(checkout.path);
        }
    }
    Err(WorktreeError::refused(
        "branch_not_held",
        &format!("no checkout holds branch {branch}"),
    ))
}

/// Finds free slots idle for 30 days and output bases whose workspace is gone.
pub(crate) fn prune_plan(pool: &Pool, host: &dyn Host) -> PrunePlan {
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
        let mut bytes = host.disk_bytes(&path);
        for base in bases
            .iter()
            .filter(|base| same_path(&base.workspace, &path))
        {
            bytes += host.disk_bytes(&base.dir);
        }
        plan.slots.push(PruneSlot {
            slot: record.slot,
            path,
            idle_days: idle / 86_400,
            bytes,
        });
    }
    let default = git::default_base(&pool.main).ok();
    for checkout in git::checkouts(&pool.main).unwrap_or_default() {
        if same_path(&checkout.path, &pool.main)
            || pool.slot_of(&checkout.path).is_some()
            || !external_prunable(pool, &checkout, default.as_deref(), &servers)
        {
            continue;
        }
        let mut bytes = host.disk_bytes(&checkout.path);
        for base in bases
            .iter()
            .filter(|base| same_path(&base.workspace, &checkout.path))
        {
            bytes += host.disk_bytes(&base.dir);
        }
        plan.externals.push(PruneExternal {
            path: checkout.path,
            branch: checkout.branch,
            bytes,
        });
    }
    for base in &bases {
        if base.workspace.exists() || base_in_use(&servers, base) {
            continue;
        }
        plan.bases.push(PruneBase {
            dir: base.dir.clone(),
            workspace: base.workspace.clone(),
            bytes: host.disk_bytes(&base.dir),
        });
    }
    plan
}

fn base_in_use(servers: &[Server], base: &OutputBase) -> bool {
    servers.iter().any(|server| {
        server
            .output_base
            .as_deref()
            .is_some_and(|dir| same_path(dir, &base.dir))
    })
}

/// Deletes what `prune_plan` found and returns the external worktrees it kept.
pub(crate) fn prune_apply(
    pool: &Pool,
    host: &dyn Host,
    plan: &PrunePlan,
) -> Result<Vec<PruneKept>, WorktreeError> {
    let _lock = pool.lock()?;
    let bases = scan_output_bases(&host.output_roots());
    for slot in &plan.slots {
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
        remove_external(pool, host, &external.path, &bases);
    }
    for base in &plan.bases {
        remove_tree(&base.dir);
    }
    Ok(kept)
}

fn delete_slot(pool: &Pool, host: &dyn Host, slot: u32, bases: &[OutputBase]) {
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
