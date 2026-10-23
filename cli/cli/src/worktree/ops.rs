use std::path::{Path, PathBuf};

use super::git;
use super::host::{same_path, scan_output_bases, Host, OutputBase, Server};
use super::pool::{Pool, Record, Who};
use super::WorktreeError;

const RECLAIM_AFTER_SECS: u64 = 24 * 60 * 60;
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
}

/// The slot an `add` handed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Added {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) reused: bool,
    pub(crate) warnings: Vec<String>,
}

/// The slot a `release` freed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Released {
    pub(crate) slot: u32,
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) already_free: bool,
}

/// One row of `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) slot: Option<u32>,
    pub(crate) path: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) holder: Option<String>,
    pub(crate) state: State,
    pub(crate) server: bool,
    pub(crate) disk_bytes: u64,
}

/// The slots plus the machine totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listing {
    pub(crate) rows: Vec<Row>,
    pub(crate) servers: u64,
    pub(crate) servers_rss_bytes: u64,
    pub(crate) disk_bytes: u64,
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

/// What `prune` found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PrunePlan {
    pub(crate) slots: Vec<PruneSlot>,
    pub(crate) bases: Vec<PruneBase>,
}

impl PrunePlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.slots.is_empty() && self.bases.is_empty()
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.slots.iter().map(|slot| slot.bytes).sum::<u64>()
            + self.bases.iter().map(|base| base.bytes).sum::<u64>()
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn holder_alive(host: &dyn Host, record: &Record) -> bool {
    record.pid.is_some_and(|pid| host.process_alive(pid))
}

fn reclaimable(pool: &Pool, host: &dyn Host, record: &Record) -> bool {
    if !record.held || holder_alive(host, record) {
        return false;
    }
    if host.now().saturating_sub(record.leased_at) < RECLAIM_AFTER_SECS {
        return false;
    }
    let path = pool.slot_path(record.slot);
    matches!(git::dirty(&path), Ok(false)) && matches!(git::unpushed(&path), Ok(0))
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
        let base = if requested.is_empty() {
            git::default_base(main)?
        } else {
            requested.to_owned()
        };
        if !git::ref_exists(main, &base) {
            return Err(WorktreeError::usage(
                "unknown_ref",
                &format!("--create base {base} does not name a commit"),
            ));
        }
        return Ok(Start::Create(base));
    }
    if git::branch_exists(main, branch) {
        return Ok(Start::Local);
    }
    if git::ref_exists(main, &format!("origin/{branch}")) {
        return Ok(Start::Remote);
    }
    Err(WorktreeError::usage(
        "unknown_branch",
        &format!("branch {branch} does not exist; use --create to start it"),
    ))
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
            warnings.push(format!("cannot fetch origin, using local refs: {error}"));
        }
    }
    let lock = pool.lock()?;
    pool.forget_missing();
    let now = host.now();
    let records = pool.records();
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
        return Err(WorktreeError::refused(
            "branch_held",
            &format!(
                "branch {branch} is held by slot {} at {}, leased to {}",
                record.slot,
                path.display(),
                describe_holder(host, record)
            ),
        ));
    }
    for checkout in git::checkouts(&pool.main)? {
        if checkout.branch.as_deref() == Some(branch) {
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
    let mut free: Vec<&Record> = records.iter().filter(|record| !record.held).collect();
    rank(&mut free, branch, digest.as_deref(), low_disk);
    let mut reclaimed = false;
    let mut chosen: Option<&Record> = free.first().copied();
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
        pid: Some(who.pid),
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
            freed.held = false;
            freed.branch = None;
            freed.session = None;
            freed.pid = None;
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
    let slot = pool.slot_of(path).ok_or_else(|| {
        WorktreeError::usage(
            "not_a_slot",
            &format!("{} is not a dx worktree slot", path.display()),
        )
    })?;
    let record = pool.record(slot).ok_or_else(|| {
        WorktreeError::usage("not_a_slot", &format!("slot {slot} has no lease record"))
    })?;
    let slot_path = pool.slot_path(slot);
    if !record.held {
        return Ok(Released {
            slot,
            path: slot_path,
            branch: None,
            already_free: true,
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
    freed.held = false;
    freed.last_branch = record.branch.clone().or(record.last_branch.clone());
    freed.branch = None;
    freed.session = None;
    freed.pid = None;
    freed.last_used = host.now();
    pool.write(&freed)?;
    Ok(Released {
        slot,
        path: slot_path,
        branch: record.branch,
        already_free: false,
    })
}

fn server_in(servers: &[Server], path: &Path) -> bool {
    servers.iter().any(|server| {
        server
            .workspace
            .as_deref()
            .is_some_and(|workspace| same_path(workspace, path))
    })
}

/// Lists every slot and hand-made checkout, with machine totals.
pub(crate) fn list(pool: &Pool, host: &dyn Host) -> Result<Listing, WorktreeError> {
    pool.forget_missing();
    let servers = host.servers();
    let bases = scan_output_bases(&host.output_roots());
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
        let mut disk_bytes = host.disk_bytes(&path);
        for base in bases
            .iter()
            .filter(|base| same_path(&base.workspace, &path))
        {
            disk_bytes += host.disk_bytes(&base.dir);
        }
        slot_disk += disk_bytes;
        rows.push(Row {
            slot: Some(record.slot),
            branch: git::current_branch(&path).or(record.branch.clone()),
            holder: record.holder(holder_alive(host, &record)),
            state,
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
            server: server_in(&servers, &checkout.path),
            disk_bytes: host.disk_bytes(&checkout.path),
            path: checkout.path,
        });
    }
    Ok(Listing {
        rows,
        servers: servers.len() as u64,
        servers_rss_bytes: servers.iter().map(|server| server.rss_bytes).sum(),
        disk_bytes: slot_disk,
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

/// Deletes what `prune_plan` found.
pub(crate) fn prune_apply(
    pool: &Pool,
    host: &dyn Host,
    plan: &PrunePlan,
) -> Result<(), WorktreeError> {
    let _lock = pool.lock()?;
    let bases = scan_output_bases(&host.output_roots());
    for slot in &plan.slots {
        if pool.record(slot.slot).is_some_and(|record| !record.held) {
            delete_slot(pool, host, slot.slot, &bases);
        }
    }
    for base in &plan.bases {
        remove_tree(&base.dir);
    }
    Ok(())
}

fn delete_slot(pool: &Pool, host: &dyn Host, slot: u32, bases: &[OutputBase]) {
    let path = pool.slot_path(slot);
    host.bazel_shutdown(&path);
    let arg = path_text(&path);
    let _ = git::run(&pool.main, &["worktree", "unlock", &arg]);
    let _ = git::run(&pool.main, &["worktree", "remove", "--force", &arg]);
    remove_tree(&path);
    let _ = git::run(&pool.main, &["worktree", "prune"]);
    pool.remove(slot);
    for base in bases
        .iter()
        .filter(|base| same_path(&base.workspace, &path))
    {
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
