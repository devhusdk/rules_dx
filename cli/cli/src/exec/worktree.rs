use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::common::*;
use dx_output::{
    command_finished, command_started, notice_event, slot_event, summary_event, FinishedCounts,
    NoticeEvent, OutputMode, SlotEvent, SummaryEvent,
};
use dx_plan::args::Invocation;
use dx_worktree::host::{Host, SystemHost};
use dx_worktree::ops::{self, size, AddRequest, Added, Listing, PrunePlan, Released, Row, Warning};
use dx_worktree::pool::{cache_base, Pool, Who};
use dx_worktree::{hook, HookKind, WorktreeError};

const CODE_WORKTREE_FAILED: &str = "worktree_failed";

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn open_pool(dir: &Path) -> Result<Pool, WorktreeError> {
    let base = cache_base(&env_var)?;
    Pool::open(dir, &base)
}

fn row_event(action: &str, row: &Row) -> serde_json::Value {
    slot_event(&SlotEvent {
        action: action.to_owned(),
        path: row.path.to_string_lossy().into_owned(),
        slot: row.slot,
        branch: row.branch.clone(),
        holder: row.holder.clone(),
        state: row.state.name().to_owned(),
        reason: row.reason.map(str::to_owned),
        base: None,
        server: row.server,
        disk_bytes: row.disk_bytes,
    })
    .unwrap_or(serde_json::Value::Null)
}

fn simple_event(
    action: &str,
    slot: Option<u32>,
    path: &Path,
    branch: Option<String>,
    state: &str,
) -> serde_json::Value {
    slot_event(&SlotEvent {
        action: action.to_owned(),
        path: path.to_string_lossy().into_owned(),
        slot,
        branch,
        holder: None,
        state: state.to_owned(),
        reason: None,
        base: None,
        server: false,
        disk_bytes: None,
    })
    .unwrap_or(serde_json::Value::Null)
}

fn write_line(out: &mut dyn Write, text: &str) -> Result<(), i32> {
    check_stdout_write(writeln!(out, "{text}"))
}

fn start(out: &mut dyn Write, invocation: &Invocation, mode: &str) -> Result<(), i32> {
    if invocation.output == OutputMode::Json {
        emit_built(out, command_started(invocation.command.name(), mode))?;
    }
    Ok(())
}

fn finish(out: &mut dyn Write, invocation: &Invocation, code: i32) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default())) {
            return exit;
        }
    }
    code
}

fn fail(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    error: &WorktreeError,
) -> i32 {
    let code = if error.signal.is_some() {
        "cancelled"
    } else if error.usage {
        "invalid_arguments"
    } else {
        CODE_WORKTREE_FAILED
    };
    report_failure(
        invocation.output,
        out,
        err,
        &Failure::new(code, &error.message, error.exit()),
    )
}

/// Runs one `dx worktree` subcommand.
pub(crate) fn execute_worktree(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    let host = SystemHost;
    let who = Who::from_env(&env_var);
    let mut words: Vec<&str> = invocation.targets.iter().map(String::as_str).collect();
    if words.is_empty() {
        words.push("list");
    }
    if let Some(flag) = misplaced_flag(invocation, words[0]) {
        let error = WorktreeError::usage(
            "invalid_arguments",
            &format!(
                "option {flag:?} is not supported by dx worktree {}",
                words[0]
            ),
        );
        return fail(invocation, out, err, &error);
    }
    if let ["hook", word] = words.as_slice() {
        if let Some(kind) = HookKind::from_word(word) {
            let stdin = read_stdin();
            return run_hook(kind, &stdin, workspace, &host, &who, out, err);
        }
    }
    let result = match words.as_slice() {
        ["add", branch] => add(invocation, workspace, &host, &who, branch, out, err),
        ["release"] => release(invocation, workspace, &host, &who, None, out),
        ["release", path] => release(invocation, workspace, &host, &who, Some(path), out),
        ["list"] => list(invocation, workspace, &host, out),
        ["path", branch] => path(invocation, workspace, branch, out),
        ["prune"] => prune(invocation, workspace, &host, out),
        _ => Err(Failed::Error(WorktreeError::usage(
            "invalid_arguments",
            "want one of: add, release, list, path, prune, hook",
        ))),
    };
    match result {
        Ok(code) => finish(out, invocation, code),
        Err(Failed::Exit(code)) => code,
        Err(Failed::Error(error)) => fail(invocation, out, err, &error),
    }
}

fn misplaced_flag(invocation: &Invocation, sub: &str) -> Option<&'static str> {
    if invocation.create.is_some() && sub != "add" {
        return Some("--create");
    }
    if invocation.force && sub != "release" && sub != "add" {
        return Some("--force");
    }
    if invocation.disk && sub != "list" {
        return Some("--disk");
    }
    if invocation.bases && sub != "prune" {
        return Some("--bases");
    }
    if invocation.apply && sub != "prune" {
        return Some("--apply");
    }
    if invocation.path_only && sub != "add" {
        return Some("--output=path");
    }
    None
}

enum Failed {
    Exit(i32),
    Error(WorktreeError),
}

impl From<WorktreeError> for Failed {
    fn from(error: WorktreeError) -> Failed {
        Failed::Error(error)
    }
}

impl From<i32> for Failed {
    fn from(code: i32) -> Failed {
        Failed::Exit(code)
    }
}

fn add(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    who: &Who,
    branch: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32, Failed> {
    let pool = open_pool(workspace)?;
    ops::claim(&pool, who, workspace)?;
    let added = ops::add(
        &pool,
        host,
        who,
        &AddRequest {
            branch: branch.to_owned(),
            create: invocation.create.clone(),
            force: invocation.force,
            unclaimed: false,
        },
    )?;
    add_output(out, err, invocation, branch, &added)?;
    Ok(0)
}

fn warning_events(out: &mut dyn Write, warnings: &[Warning]) -> Result<(), i32> {
    for warning in warnings {
        let notice = NoticeEvent {
            related_command: Some("worktree".to_owned()),
            ..NoticeEvent::warning(warning.code, warning.message.clone())
        };
        emit_built(out, notice_event(&notice))?;
    }
    Ok(())
}

fn add_output(
    out: &mut dyn Write,
    err: &mut dyn Write,
    invocation: &Invocation,
    branch: &str,
    added: &Added,
) -> Result<(), i32> {
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        warning_events(out, &added.warnings)?;
        let event = slot_event(&SlotEvent {
            action: "add".to_owned(),
            path: added.path.to_string_lossy().into_owned(),
            slot: Some(added.slot),
            branch: Some(branch.to_owned()),
            holder: None,
            state: "held".to_owned(),
            reason: None,
            base: added.base.clone(),
            server: false,
            disk_bytes: None,
        })
        .unwrap_or(serde_json::Value::Null);
        emit_event(out, &event)?;
    } else if invocation.path_only {
        warn_lines(err, &added.warnings);
        if let Some(base) = &added.base {
            let _ = writeln!(err, "dx worktree: started {branch} from {base}");
        }
        write_line(out, &added.path.to_string_lossy())?;
    } else {
        for warning in &added.warnings {
            write_line(out, &format!("dx worktree: {}", warning.message))?;
        }
        if let Some(base) = &added.base {
            write_line(out, &format!("dx worktree: started {branch} from {base}"))?;
        }
        write_line(
            out,
            &format!(
                "dx worktree: {branch} is in slot {} at {}",
                added.slot,
                added.path.display()
            ),
        )?;
    }
    Ok(())
}

/// Resolves a release path, reading a relative one from `run_dir`.
fn release_target(path: Option<&str>, workspace: &Path, run_dir: &Path) -> PathBuf {
    match path.map(PathBuf::from) {
        None => workspace.to_path_buf(),
        Some(path) if path.is_absolute() => path,
        Some(path) => run_dir.join(path),
    }
}

fn release(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    who: &Who,
    path: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let cwd = dx_process::current_directory().unwrap_or_else(|_| workspace.to_path_buf());
    let target = release_target(path, workspace, &cwd);
    let pool = open_pool(workspace)?;
    ops::claim(&pool, who, workspace)?;
    let released = ops::release(&pool, host, who, &target, invocation.force)?;
    release_output(out, invocation, &released)?;
    Ok(0)
}

fn release_output(
    out: &mut dyn Write,
    invocation: &Invocation,
    released: &Released,
) -> Result<(), i32> {
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        warning_events(out, &released.warnings)?;
        emit_event(
            out,
            &simple_event(
                "release",
                released.slot,
                &released.path,
                released.branch.clone(),
                if released.removed { "removed" } else { "free" },
            ),
        )?;
        return Ok(());
    }
    for warning in &released.warnings {
        write_line(out, &format!("dx worktree: {}", warning.message))?;
    }
    if released.removed {
        write_line(
            out,
            &format!("dx worktree: removed {}", released.path.display()),
        )?;
    } else if let Some(slot) = released.slot {
        if released.already_free {
            write_line(out, &format!("dx worktree: slot {slot} is already free"))?;
        } else {
            write_line(
                out,
                &format!(
                    "dx worktree: released slot {slot} at {}",
                    released.path.display()
                ),
            )?;
        }
    }
    Ok(())
}

fn list_lines(listing: &Listing) -> Vec<String> {
    let mut lines = Vec::new();
    for row in &listing.rows {
        let slot = row
            .slot
            .map(|slot| slot.to_string())
            .unwrap_or_else(|| "-".to_owned());
        let state = match row.reason {
            Some(reason) => format!("{} ({reason})", row.state.name()),
            None => row.state.name().to_owned(),
        };
        lines.push(format!(
            "{slot}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.branch.as_deref().unwrap_or("(detached)"),
            row.holder.as_deref().unwrap_or("-"),
            state,
            if row.server { "server" } else { "no server" },
            row.disk_bytes.map(size).unwrap_or_else(|| "-".to_owned()),
            row.path.display()
        ));
    }
    let mut totals = format!(
        "{} bazel server(s) running, {} resident",
        listing.servers,
        size(listing.servers_rss_bytes)
    );
    if let Some(bytes) = listing.disk_bytes {
        totals.push_str(&format!(", {} across slots", size(bytes)));
    }
    if let Some(bytes) = listing.disk_cache_bytes {
        totals.push_str(&format!(", {} disk cache", size(bytes)));
    }
    lines.push(totals);
    lines
}

fn list(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let pool = open_pool(workspace)?;
    let listing = ops::list(&pool, host, invocation.disk)?;
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        let disk_cache_bytes = match listing.disk_cache_bytes {
            Some(bytes) => bytes,
            None => ops::disk_cache_bytes(host)?,
        };
        for row in &listing.rows {
            emit_event(out, &row_event("list", row))?;
        }
        emit_event(
            out,
            &summary_event(&SummaryEvent {
                slots: listing.rows.iter().filter(|row| row.slot.is_some()).count() as u64,
                servers: listing.servers,
                servers_rss_bytes: listing.servers_rss_bytes,
                disk_bytes: listing.disk_bytes,
                disk_cache_bytes: Some(disk_cache_bytes),
            }),
        )?;
    } else {
        for line in list_lines(&listing) {
            write_line(out, &line)?;
        }
    }
    Ok(0)
}

fn path(
    invocation: &Invocation,
    workspace: &Path,
    branch: &str,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let pool = open_pool(workspace)?;
    let found = ops::path_of(&pool, branch)?;
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        emit_event(
            out,
            &simple_event(
                "path",
                pool.slot_of(&found),
                &found,
                Some(branch.to_owned()),
                "held",
            ),
        )?;
    } else {
        write_line(out, &found.to_string_lossy())?;
    }
    Ok(0)
}

fn prune_lines(plan: &PrunePlan) -> Vec<String> {
    let mut lines = Vec::new();
    for slot in &plan.slots {
        lines.push(format!(
            "slot {} idle {} days, {}: {}",
            slot.slot,
            slot.idle_days,
            size(slot.bytes),
            slot.path.display()
        ));
    }
    for external in &plan.externals {
        lines.push(format!(
            "external worktree {}, {}: {}",
            external.branch.as_deref().unwrap_or("(detached)"),
            size(external.bytes),
            external.path.display()
        ));
    }
    for base in &plan.bases {
        lines.push(match &base.owner {
            ops::BaseOwner::Slot(slot) => format!(
                "output base of free slot {slot}, {}: {}",
                size(base.bytes),
                base.dir.display()
            ),
            ops::BaseOwner::Nested { case, .. } => format!(
                "output base of nested Bazel test {case}, {}: {}",
                size(base.bytes),
                base.dir.display()
            ),
            ops::BaseOwner::Orphan => format!(
                "output base of removed {}, {}: {}",
                base.workspace.display(),
                size(base.bytes),
                base.dir.display()
            ),
        });
    }
    for cache in &plan.caches {
        lines.push(format!(
            "cache of free slot {}, {}: {}",
            cache.slot,
            size(cache.bytes),
            cache.path.display()
        ));
    }
    for downloads in &plan.downloads {
        lines.push(format!(
            "repository cache downloads unused for 30 days, {} entries, {}: {}",
            downloads.entries.len(),
            size(downloads.bytes),
            downloads.cache.display()
        ));
    }
    lines
}

fn skipped_lines(
    slots: &[ops::PruneSkippedSlot],
    bases: &[ops::PruneKeptBase],
    caches: &[ops::PruneSkippedCache],
) -> Vec<String> {
    let mut lines: Vec<String> = slots
        .iter()
        .map(|slot| {
            format!(
                "skipped slot {} ({}): {}",
                slot.slot,
                slot.reason,
                slot.path.display()
            )
        })
        .collect();
    lines.extend(bases.iter().map(|base| match &base.owner {
        ops::BaseOwner::Slot(slot) => format!(
            "skipped output base of free slot {slot} ({}): {}",
            base.reason,
            base.dir.display()
        ),
        ops::BaseOwner::Nested { case, .. } => format!(
            "skipped output base of nested Bazel test {case} ({}): {}",
            base.reason,
            base.dir.display()
        ),
        ops::BaseOwner::Orphan => format!(
            "skipped output base of removed {} ({}): {}",
            base.workspace.display(),
            base.reason,
            base.dir.display()
        ),
    }));
    lines.extend(caches.iter().map(|cache| {
        format!(
            "skipped cache of free slot {} ({}): {}",
            cache.slot,
            cache.reason,
            cache.path.display()
        )
    }));
    lines
}

/// The line for an output base `prune` keeps because a server still uses it.
fn kept_base_line(base: &ops::PruneKeptBase) -> String {
    match &base.owner {
        ops::BaseOwner::Nested { case, .. } => format!(
            "kept output base of nested Bazel test {case} ({}): {}",
            base.reason,
            base.dir.display()
        ),
        _ => format!(
            "kept output base of removed {} ({}): {}",
            base.workspace.display(),
            base.reason,
            base.dir.display()
        ),
    }
}

fn prune(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let pool = open_pool(workspace)?;
    let pruned = ops::prune(&pool, host, invocation.applies(), invocation.bases)?;
    Ok(prune_output(out, invocation, &pruned)?)
}

fn prune_events(pruned: &ops::Pruned) -> Vec<serde_json::Value> {
    let ops::Pruned {
        plan,
        kept,
        skipped_slots,
        skipped_bases,
        skipped_caches,
    } = pruned;
    let mut events = Vec::new();
    for slot in &plan.slots {
        let mut event = simple_event("prune", Some(slot.slot), &slot.path, None, "free");
        event["disk_bytes"] = slot.bytes.into();
        events.push(event);
    }
    for slot in skipped_slots {
        let mut event = simple_event("prune", Some(slot.slot), &slot.path, None, "skipped");
        event["reason"] = slot.reason.into();
        events.push(event);
    }
    for external in &plan.externals {
        let mut event = simple_event(
            "prune",
            None,
            &external.path,
            external.branch.clone(),
            "external",
        );
        event["disk_bytes"] = external.bytes.into();
        events.push(event);
    }
    for item in plan.kept_externals.iter().chain(kept) {
        let mut event = simple_event("prune", None, &item.path, item.branch.clone(), "kept");
        event["reason"] = item.reason.into();
        events.push(event);
    }
    for base in &plan.kept_bases {
        let mut event = simple_event("prune_base", None, &base.dir, None, "kept");
        event["reason"] = base.reason.into();
        events.push(event);
    }
    for base in skipped_bases {
        let mut event = simple_event("prune_base", base.owner.slot(), &base.dir, None, "skipped");
        event["reason"] = base.reason.into();
        events.push(event);
    }
    for base in &plan.bases {
        let state = match base.owner {
            ops::BaseOwner::Slot(_) => "free",
            ops::BaseOwner::Nested { .. } => "nested",
            ops::BaseOwner::Orphan => "orphaned",
        };
        let mut event = simple_event("prune_base", base.owner.slot(), &base.dir, None, state);
        event["disk_bytes"] = base.bytes.into();
        events.push(event);
    }
    for cache in skipped_caches {
        let mut event = simple_event(
            "prune_cache",
            Some(cache.slot),
            &cache.path,
            None,
            "skipped",
        );
        event["reason"] = cache.reason.into();
        events.push(event);
    }
    for cache in &plan.caches {
        let mut event = simple_event("prune_cache", Some(cache.slot), &cache.path, None, "free");
        event["disk_bytes"] = cache.bytes.into();
        events.push(event);
    }
    for downloads in &plan.downloads {
        let mut event = simple_event("prune_downloads", None, &downloads.cache, None, "stale");
        event["disk_bytes"] = downloads.bytes.into();
        event["entries"] = downloads.entries.len().into();
        events.push(event);
    }
    events
}

fn prune_output(
    out: &mut dyn Write,
    invocation: &Invocation,
    pruned: &ops::Pruned,
) -> Result<i32, i32> {
    let apply = invocation.applies();
    let ops::Pruned {
        plan,
        kept,
        skipped_slots,
        skipped_bases,
        skipped_caches,
    } = pruned;
    start(out, invocation, if apply { "default" } else { "check" })?;
    if invocation.output == OutputMode::Json {
        for event in prune_events(pruned) {
            emit_event(out, &event)?;
        }
        return Ok(prune_exit(apply, plan));
    }
    for base in &plan.kept_bases {
        write_line(out, &kept_base_line(base))?;
    }
    for item in &plan.kept_externals {
        write_line(out, &kept_external_line(item))?;
    }
    if plan.is_empty()
        && kept.is_empty()
        && skipped_slots.is_empty()
        && skipped_bases.is_empty()
        && skipped_caches.is_empty()
    {
        write_line(out, "dx worktree: nothing to prune")?;
    } else {
        for line in skipped_lines(skipped_slots, skipped_bases, skipped_caches) {
            write_line(out, &line)?;
        }
        for item in kept {
            write_line(out, &kept_external_line(item))?;
        }
        for line in prune_lines(plan) {
            write_line(out, &line)?;
        }
        write_line(
            out,
            &if apply {
                format!("dx worktree: pruned {}", size(plan.bytes()))
            } else {
                format!(
                    "dx worktree: {} to reclaim; run `dx worktree prune --apply`",
                    size(plan.bytes())
                )
            },
        )?;
    }
    Ok(prune_exit(apply, plan))
}

fn kept_external_line(item: &ops::PruneKept) -> String {
    format!(
        "kept external worktree {} ({}): {}",
        item.branch.as_deref().unwrap_or("(detached)"),
        item.reason,
        item.path.display()
    )
}

fn prune_exit(apply: bool, plan: &PrunePlan) -> i32 {
    if apply || plan.is_empty() {
        0
    } else {
        1
    }
}

fn read_stdin() -> String {
    std::io::read_to_string(std::io::stdin()).unwrap_or_default()
}

/// The hook kind for an argv that is exactly `worktree hook <kind>`.
pub fn hook_kind<S: AsRef<OsStr>>(args: &[S]) -> Option<HookKind> {
    match args {
        [command, sub, word] if command.as_ref() == "worktree" && sub.as_ref() == "hook" => {
            word.as_ref().to_str().and_then(HookKind::from_word)
        }
        _ => None,
    }
}

/// Runs one agent hook without loading dx defaults.
pub fn run_worktree_hook(
    kind: HookKind,
    workspace: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let stdin = read_stdin();
    if kind == HookKind::Guard {
        return guard_hook(&stdin, out, err);
    }
    dx_process::configure_real(workspace);
    let who = Who::from_env(&env_var);
    run_hook(kind, &stdin, workspace, &SystemHost, &who, out, err)
}

fn warn_lines(err: &mut dyn Write, warnings: &[Warning]) {
    for warning in warnings {
        let _ = writeln!(err, "dx worktree: {}", warning.message);
    }
}

fn create_output(out: &mut dyn Write, err: &mut dyn Write, added: &Added) -> i32 {
    warn_lines(err, &added.warnings);
    match write_line(out, &added.path.to_string_lossy()) {
        Ok(()) => 0,
        Err(exit) => exit,
    }
}

fn run_hook(
    kind: HookKind,
    stdin: &str,
    workspace: &Path,
    host: &dyn Host,
    who: &Who,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match kind {
        HookKind::Guard => guard_hook(stdin, out, err),
        HookKind::Create => pool_hook(kind, stdin, workspace, err, |pool, input, err| {
            hook::create(pool, host, who, input).map(|added| create_output(out, err, &added))
        }),
        HookKind::Stop => pool_hook(kind, stdin, workspace, err, |pool, input, _| {
            hook::stop(pool, who, input).map(|_| 0)
        }),
        HookKind::Remove => pool_hook(kind, stdin, workspace, err, |pool, input, _| {
            hook::remove(pool, host, who, input).map(|_| 0)
        }),
    }
}

fn guard_hook(stdin: &str, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let input = match hook::parse(stdin) {
        Ok(input) => input,
        Err(error) => {
            let _ = writeln!(err, "dx worktree: hook guard: {error}; command not checked");
            return 0;
        }
    };
    match hook::guard(&input) {
        hook::Guard::Block(line) => {
            let _ = writeln!(err, "{line}");
            dx_process::pre_exec_code()
        }
        hook::Guard::Rewrite(output) => match write_line(out, &output.to_string()) {
            Ok(()) => 0,
            Err(exit) => exit,
        },
        hook::Guard::Allow => 0,
    }
}

fn pool_hook(
    kind: HookKind,
    stdin: &str,
    workspace: &Path,
    err: &mut dyn Write,
    op: impl FnOnce(&Pool, &serde_json::Value, &mut dyn Write) -> Result<i32, WorktreeError>,
) -> i32 {
    let result = hook::parse(stdin).and_then(|input| {
        if hook::ignored(kind, &input) {
            return Ok(0);
        }
        let dir = hook::cwd(&input).unwrap_or_else(|| workspace.to_path_buf());
        open_pool(&dir).and_then(|pool| op(&pool, &input, &mut *err))
    });
    match result {
        Ok(code) => code,
        Err(error) => hook_error(err, &error),
    }
}

fn hook_error(err: &mut dyn Write, error: &WorktreeError) -> i32 {
    let _ = writeln!(err, "dx worktree: {error}");
    dx_process::operational_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::parsed;
    use dx_worktree::host::Download;
    use dx_worktree::ops::State;

    #[test]
    fn bases_is_rejected_on_every_subcommand_but_prune() {
        for words in [
            &["worktree", "add", "x", "--bases"][..],
            &["worktree", "release", "--bases"],
            &["worktree", "list", "--bases"],
            &["worktree", "path", "x", "--bases"],
        ] {
            assert_eq!(
                misplaced_flag(&parsed(words), words[1]),
                Some("--bases"),
                "{words:?}"
            );
        }
        assert_eq!(
            misplaced_flag(&parsed(&["worktree", "prune", "--bases"]), "prune"),
            None
        );
    }

    #[test]
    fn disk_is_rejected_on_every_subcommand_but_list() {
        for words in [
            &["worktree", "prune", "--disk"][..],
            &["worktree", "release", "--disk"],
            &["worktree", "path", "x", "--disk"],
            &["worktree", "add", "x", "--disk"],
        ] {
            assert_eq!(
                misplaced_flag(&parsed(words), words[1]),
                Some("--disk"),
                "{words:?}"
            );
        }
        assert_eq!(
            misplaced_flag(&parsed(&["worktree", "list", "--disk"]), "list"),
            None
        );
        assert_eq!(
            misplaced_flag(&parsed(&["worktree", "--disk"]), "list"),
            None
        );
        let harness = crate::exec::test_support::Harness::new("wt_disk");
        let (code, _, stderr) = harness.run(&["worktree", "prune", "--disk"]);
        assert_eq!(code, 2, "{stderr}");
        assert!(
            stderr.contains("option \"--disk\" is not supported by dx worktree prune"),
            "{stderr}"
        );
    }

    #[test]
    fn misplaced_flag_under_json_reports_invalid_arguments() {
        let harness = crate::exec::test_support::Harness::new("wt_misplaced_json");
        let (code, stdout, stderr) = harness.run(&["worktree", "list", "--apply", "--output=json"]);
        assert_eq!(code, 2, "{stderr}");
        assert!(
            stderr.contains("option \"--apply\" is not supported by dx worktree list"),
            "{stderr}"
        );
        let events: Vec<serde_json::Value> = stdout
            .lines()
            .map(|line| serde_json::from_str(line).expect("json line"))
            .collect();
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "error" && event["code"] == "invalid_arguments"),
            "{stdout}"
        );
        let last = events.last().expect("events");
        assert_eq!(last["event"], "command_finished", "{stdout}");
        assert_eq!(last["exit_code"], 2, "{stdout}");
        assert!(
            !events
                .iter()
                .any(|event| event["event"] == "command_started"),
            "{stdout}"
        );
    }

    fn listing(disk: Option<u64>) -> Listing {
        Listing {
            rows: vec![Row {
                slot: Some(1),
                path: PathBuf::from("/path"),
                branch: Some("b".to_owned()),
                holder: None,
                state: State::Free,
                reason: None,
                server: false,
                disk_bytes: disk,
            }],
            servers: 1,
            servers_rss_bytes: 1024,
            disk_bytes: disk,
            disk_cache_bytes: None,
        }
    }

    #[test]
    fn list_lines_show_a_dash_and_no_disk_total_without_disk() {
        let lines = list_lines(&listing(None));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\t-\t/path"), "{}", lines[0]);
        assert_eq!(lines[1], "1 bazel server(s) running, 1.0 KiB resident");
    }

    #[test]
    fn list_lines_show_sizes_and_the_disk_total_with_disk() {
        let lines = list_lines(&listing(Some(8192)));
        assert!(lines[0].contains("\t8.0 KiB\t/path"), "{}", lines[0]);
        assert_eq!(
            lines[1],
            "1 bazel server(s) running, 1.0 KiB resident, 8.0 KiB across slots"
        );
    }

    #[test]
    fn stopped_error_reports_cancelled_with_128_plus_signal() {
        let invocation = parsed(&["worktree", "list", "--disk"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = fail(&invocation, &mut out, &mut err, &WorktreeError::stopped(15));
        assert_eq!(code, 143);
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "dx: cancelled: stopped by signal 15\n"
        );
    }

    fn added() -> Added {
        Added {
            slot: 3,
            path: PathBuf::from("/cache/slots/3"),
            reused: false,
            base: Some("origin/main".to_owned()),
            warnings: vec![Warning {
                code: "fetch_failed",
                message: "cannot fetch origin, using local refs: offline".to_owned(),
            }],
        }
    }

    fn render(words: &[&str]) -> (String, String) {
        let invocation = parsed(words);
        let mut out = Vec::new();
        let mut err = Vec::new();
        add_output(&mut out, &mut err, &invocation, "x", &added()).expect("output");
        (
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    #[test]
    fn add_json_emits_a_warning_notice_before_the_slot() {
        let (text, stderr) = render(&["worktree", "add", "x", "--output=json"]);
        assert_eq!(stderr, "");
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds, ["command_started", "notice", "slot"], "{text}");
        assert_eq!(events[1]["level"], "warning");
        assert_eq!(events[1]["code"], "fetch_failed");
        assert_eq!(
            events[1]["message"],
            "cannot fetch origin, using local refs: offline"
        );
        assert_eq!(events[2]["action"], "add");
        assert_eq!(events[2]["state"], "held");
        assert_eq!(events[2]["base"], "origin/main");
    }

    #[test]
    fn add_without_a_new_branch_shows_no_base() {
        let reused = Added {
            reused: true,
            base: None,
            warnings: Vec::new(),
            ..added()
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let invocation = parsed(&["worktree", "add", "x"]);
        add_output(&mut out, &mut err, &invocation, "x", &reused).expect("output");
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "dx worktree: x is in slot 3 at /cache/slots/3\n"
        );
        let mut out = Vec::new();
        let invocation = parsed(&["worktree", "add", "x", "--output=json"]);
        add_output(&mut out, &mut err, &invocation, "x", &reused).expect("output");
        let text = String::from_utf8(out).expect("utf8");
        let slot: serde_json::Value =
            serde_json::from_str(text.lines().last().expect("slot")).expect("json");
        assert_eq!(slot["event"], "slot");
        assert!(slot.get("base").is_none(), "{text}");
        let mut out = Vec::new();
        let invocation = parsed(&["worktree", "add", "x", "--output=path"]);
        add_output(&mut out, &mut err, &invocation, "x", &reused).expect("output");
        assert_eq!(String::from_utf8(out).expect("utf8"), "/cache/slots/3\n");
        assert_eq!(String::from_utf8(err).expect("utf8"), "");
    }

    #[test]
    fn add_text_prints_the_warning_before_the_slot_line() {
        let (text, stderr) = render(&["worktree", "add", "x"]);
        assert_eq!(stderr, "");
        assert_eq!(
            text,
            "dx worktree: cannot fetch origin, using local refs: offline\n\
             dx worktree: started x from origin/main\n\
             dx worktree: x is in slot 3 at /cache/slots/3\n"
        );
    }

    #[test]
    fn add_path_prints_the_path_and_warns_on_stderr() {
        let (text, stderr) = render(&["worktree", "add", "x", "--output=path"]);
        assert_eq!(text, "/cache/slots/3\n");
        assert_eq!(
            stderr,
            "dx worktree: cannot fetch origin, using local refs: offline\n\
             dx worktree: started x from origin/main\n"
        );
    }

    #[test]
    fn add_path_writes_every_warning_to_stderr_in_order() {
        let invocation = parsed(&["worktree", "add", "x", "--output=path"]);
        let mut added = added();
        added.warnings.push(Warning {
            code: "disk_low",
            message: "low disk: 1 GiB free; removed 2 output bases; 9 GiB free now".to_owned(),
        });
        let mut out = Vec::new();
        let mut err = Vec::new();
        add_output(&mut out, &mut err, &invocation, "x", &added).expect("output");
        assert_eq!(String::from_utf8(out).expect("utf8"), "/cache/slots/3\n");
        assert_eq!(
            String::from_utf8(err).expect("utf8"),
            "dx worktree: cannot fetch origin, using local refs: offline\n\
             dx worktree: low disk: 1 GiB free; removed 2 output bases; 9 GiB free now\n\
             dx worktree: started x from origin/main\n"
        );
    }

    fn render_create(added: &Added) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = create_output(&mut out, &mut err, added);
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    #[test]
    fn hook_create_prints_only_the_path_and_warns_on_stderr() {
        let (code, text, stderr) = render_create(&added());
        assert_eq!(code, 0);
        assert_eq!(text, "/cache/slots/3\n");
        assert_eq!(
            stderr,
            "dx worktree: cannot fetch origin, using local refs: offline\n"
        );
    }

    #[test]
    fn hook_create_writes_every_warning_in_order_and_none_without_warnings() {
        let mut warned = added();
        warned.warnings.push(Warning {
            code: "disk_low",
            message: "low disk: 1 GiB free; removed 2 output bases; 9 GiB free now".to_owned(),
        });
        let (code, text, stderr) = render_create(&warned);
        assert_eq!(code, 0);
        assert_eq!(text, "/cache/slots/3\n");
        assert_eq!(
            stderr,
            "dx worktree: cannot fetch origin, using local refs: offline\n\
             dx worktree: low disk: 1 GiB free; removed 2 output bases; 9 GiB free now\n"
        );
        let quiet = Added {
            warnings: Vec::new(),
            ..added()
        };
        let (code, text, stderr) = render_create(&quiet);
        assert_eq!(code, 0);
        assert_eq!(text, "/cache/slots/3\n");
        assert_eq!(stderr, "");
    }

    #[test]
    fn release_target_joins_a_relative_path_onto_the_run_dir() {
        let target = release_target(
            Some("../slots/4"),
            Path::new("/ws"),
            Path::new("/home/u/proj"),
        );
        assert_eq!(target, PathBuf::from("/home/u/proj/../slots/4"));
    }

    #[test]
    fn release_target_keeps_an_absolute_path() {
        let target = release_target(
            Some("/cache/slots/4"),
            Path::new("/ws"),
            Path::new("/home/u/proj"),
        );
        assert_eq!(target, PathBuf::from("/cache/slots/4"));
    }

    #[test]
    fn release_target_defaults_to_the_workspace() {
        let target = release_target(None, Path::new("/ws"), Path::new("/home/u/proj"));
        assert_eq!(target, PathBuf::from("/ws"));
    }

    #[test]
    fn release_target_reads_relative_paths_from_the_bazel_run_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let run_dir =
            dx_process::bazel_run_dir(Some(dir.path().as_os_str()), Path::new("/runfiles"));
        let target = release_target(Some("slots/4"), Path::new("/ws"), &run_dir);
        let base = dir.path().canonicalize().expect("canonical");
        assert_eq!(target, base.join("slots/4"));
        assert!(!target.starts_with("/runfiles"), "{}", target.display());
    }

    fn render_release(words: &[&str]) -> String {
        let invocation = parsed(words);
        let released = Released {
            slot: Some(4),
            path: PathBuf::from("/cache/slots/4"),
            branch: Some("x".to_owned()),
            already_free: false,
            removed: false,
            warnings: vec![Warning {
                code: "disk_low",
                message: "low disk: 5.0 GiB free; removed the output bases of free slot(s) 2; 30.0 GiB free now".to_owned(),
            }],
        };
        let mut out = Vec::new();
        release_output(&mut out, &invocation, &released).expect("output");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn release_json_emits_the_disk_low_notice_before_the_release_event() {
        let text = render_release(&["worktree", "release", "--output=json"]);
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds, ["command_started", "notice", "slot"], "{text}");
        assert_eq!(events[1]["level"], "warning");
        assert_eq!(events[1]["code"], "disk_low");
        assert_eq!(events[2]["action"], "release");
        assert_eq!(events[2]["state"], "free");
    }

    #[test]
    fn release_text_prints_the_disk_low_line_before_the_release_line() {
        let text = render_release(&["worktree", "release"]);
        assert_eq!(
            text,
            "dx worktree: low disk: 5.0 GiB free; removed the output bases of free slot(s) 2; 30.0 GiB free now\n\
             dx worktree: released slot 4 at /cache/slots/4\n"
        );
    }

    #[test]
    fn prune_lines_name_free_slot_and_orphaned_output_bases() {
        let plan = PrunePlan {
            bases: vec![
                ops::PruneBase {
                    dir: PathBuf::from("/bazel/aaa"),
                    workspace: PathBuf::from("/gone"),
                    bytes: 2048,
                    owner: ops::BaseOwner::Orphan,
                },
                ops::PruneBase {
                    dir: PathBuf::from("/bazel/bbb"),
                    workspace: PathBuf::from("/cache/slots/2"),
                    bytes: 1024,
                    owner: ops::BaseOwner::Slot(2),
                },
                nested_base(),
            ],
            ..PrunePlan::default()
        };
        assert_eq!(
            prune_lines(&plan),
            [
                "output base of removed /gone, 2.0 KiB: /bazel/aaa",
                "output base of free slot 2, 1.0 KiB: /bazel/bbb",
                "output base of nested Bazel test hello, 512 B: /tmp/dxnb-1000/0/u/nnn",
            ]
        );
    }

    fn nested_owner() -> ops::BaseOwner {
        ops::BaseOwner::Nested {
            slot: PathBuf::from("/tmp/dxnb-1000/0"),
            case: "hello".to_owned(),
        }
    }

    fn nested_base() -> ops::PruneBase {
        ops::PruneBase {
            dir: PathBuf::from("/tmp/dxnb-1000/0/u/nnn"),
            workspace: PathBuf::from("/tmp/dxnb-1000/0/w/hello"),
            bytes: 512,
            owner: nested_owner(),
        }
    }

    #[test]
    fn kept_nested_bases_name_the_test_case() {
        let kept = ops::PruneKeptBase {
            dir: PathBuf::from("/tmp/dxnb-1000/0/u/nnn"),
            workspace: PathBuf::from("/tmp/dxnb-1000/0/w/hello"),
            owner: nested_owner(),
            reason: "server",
        };
        assert_eq!(
            kept_base_line(&kept),
            "kept output base of nested Bazel test hello (server): /tmp/dxnb-1000/0/u/nnn"
        );
    }

    #[test]
    fn caches_print_slot_size_and_skip_reason() {
        let plan = PrunePlan {
            caches: vec![ops::PruneCache {
                slot: 2,
                path: PathBuf::from("/cache/slots/2/.dx/c"),
                bytes: 1024,
            }],
            ..PrunePlan::default()
        };
        assert_eq!(
            prune_lines(&plan),
            ["cache of free slot 2, 1.0 KiB: /cache/slots/2/.dx/c"]
        );
        let skipped = [ops::PruneSkippedCache {
            slot: 2,
            path: PathBuf::from("/cache/slots/2/.dx/c"),
            reason: "held",
        }];
        assert_eq!(
            skipped_lines(&[], &[], &skipped),
            ["skipped cache of free slot 2 (held): /cache/slots/2/.dx/c"]
        );
    }

    #[test]
    fn skipped_bases_print_slot_and_reason() {
        let skipped = [
            ops::PruneKeptBase {
                dir: PathBuf::from("/bazel/bbb"),
                workspace: PathBuf::from("/cache/slots/2"),
                owner: ops::BaseOwner::Slot(2),
                reason: "server",
            },
            ops::PruneKeptBase {
                dir: PathBuf::from("/tmp/dxnb-1000/0/u/nnn"),
                workspace: PathBuf::from("/tmp/dxnb-1000/0/w/hello"),
                owner: nested_owner(),
                reason: "busy",
            },
        ];
        assert_eq!(
            skipped_lines(&[], &skipped, &[]),
            [
                "skipped output base of free slot 2 (server): /bazel/bbb",
                "skipped output base of nested Bazel test hello (busy): /tmp/dxnb-1000/0/u/nnn",
            ]
        );
    }

    #[test]
    fn skipped_slots_and_orphaned_bases_print_reason() {
        let slots = [ops::PruneSkippedSlot {
            slot: 4,
            path: PathBuf::from("/cache/slots/4"),
            reason: "busy",
        }];
        let bases = [ops::PruneKeptBase {
            dir: PathBuf::from("/bazel/ooo"),
            workspace: PathBuf::from("/gone"),
            owner: ops::BaseOwner::Orphan,
            reason: "busy",
        }];
        assert_eq!(
            skipped_lines(&slots, &bases, &[]),
            [
                "skipped slot 4 (busy): /cache/slots/4",
                "skipped output base of removed /gone (busy): /bazel/ooo"
            ]
        );
    }

    fn pruned() -> ops::Pruned {
        ops::Pruned {
            plan: PrunePlan {
                slots: vec![ops::PruneSlot {
                    slot: 3,
                    path: PathBuf::from("/cache/slots/3"),
                    idle_days: 40,
                    bytes: 100,
                }],
                caches: Vec::new(),
                externals: vec![ops::PruneExternal {
                    path: PathBuf::from("/ext"),
                    branch: Some("e".to_owned()),
                    bytes: 200,
                }],
                bases: vec![
                    ops::PruneBase {
                        dir: PathBuf::from("/bazel/bbb"),
                        workspace: PathBuf::from("/cache/slots/2"),
                        bytes: 1024,
                        owner: ops::BaseOwner::Slot(2),
                    },
                    ops::PruneBase {
                        dir: PathBuf::from("/bazel/aaa"),
                        workspace: PathBuf::from("/gone"),
                        bytes: 2048,
                        owner: ops::BaseOwner::Orphan,
                    },
                    nested_base(),
                ],
                kept_bases: vec![ops::PruneKeptBase {
                    dir: PathBuf::from("/bazel/kkk"),
                    workspace: PathBuf::from("/moved"),
                    owner: ops::BaseOwner::Orphan,
                    reason: "locked",
                }],
                kept_externals: vec![
                    ops::PruneKept {
                        path: PathBuf::from("/agent"),
                        branch: None,
                        reason: "recent",
                    },
                    ops::PruneKept {
                        path: PathBuf::from("/open"),
                        branch: Some("o".to_owned()),
                        reason: "unmerged",
                    },
                ],
                downloads: vec![ops::PruneDownloads {
                    cache: PathBuf::from("/bazel/cache/repos/v1/content_addressable"),
                    entries: ["aaa", "bbb"]
                        .iter()
                        .map(|key| Download {
                            dir: PathBuf::from(format!(
                                "/bazel/cache/repos/v1/content_addressable/sha256/{key}"
                            )),
                            cache: PathBuf::from("/bazel/cache/repos/v1/content_addressable"),
                            used: 0,
                            bytes: 512,
                        })
                        .collect(),
                    bytes: 1024,
                }],
            },
            kept: vec![ops::PruneKept {
                path: PathBuf::from("/kept"),
                branch: Some("k".to_owned()),
                reason: "server",
            }],
            skipped_slots: vec![ops::PruneSkippedSlot {
                slot: 4,
                path: PathBuf::from("/cache/slots/4"),
                reason: "busy",
            }],
            skipped_bases: vec![ops::PruneKeptBase {
                dir: PathBuf::from("/bazel/sss"),
                workspace: PathBuf::from("/cache/slots/2"),
                owner: ops::BaseOwner::Slot(2),
                reason: "server",
            }],
            skipped_caches: Vec::new(),
        }
    }

    #[test]
    fn prune_exit_is_1_only_for_pending_work_without_apply() {
        let full = pruned().plan;
        let empty = PrunePlan::default();
        assert_eq!(prune_exit(false, &full), 1);
        assert_eq!(prune_exit(false, &empty), 0);
        assert_eq!(prune_exit(true, &full), 0);
        assert_eq!(prune_exit(true, &empty), 0);
        let only_kept = PrunePlan {
            kept_bases: full.kept_bases.clone(),
            kept_externals: full.kept_externals.clone(),
            ..PrunePlan::default()
        };
        assert_eq!(prune_exit(false, &only_kept), 0);
        let only_downloads = PrunePlan {
            downloads: full.downloads.clone(),
            ..PrunePlan::default()
        };
        assert_eq!(prune_exit(false, &only_downloads), 1);
        assert_eq!(prune_exit(true, &only_downloads), 0);
    }

    #[test]
    fn cache_events_carry_slot_state_and_bytes() {
        let mut pruned = ops::Pruned::default();
        pruned.plan.caches.push(ops::PruneCache {
            slot: 2,
            path: PathBuf::from("/cache/slots/2/.dx/c"),
            bytes: 7,
        });
        pruned.skipped_caches.push(ops::PruneSkippedCache {
            slot: 3,
            path: PathBuf::from("/cache/slots/3/.dx/c"),
            reason: "busy",
        });
        let events = prune_events(&pruned);
        assert_eq!(events[0]["action"], "prune_cache");
        assert_eq!(events[0]["state"], "skipped");
        assert_eq!(events[0]["reason"], "busy");
        assert_eq!(events[1]["state"], "free");
        assert_eq!(events[1]["disk_bytes"], 7);
        assert_eq!(events[1]["slot"], 2);
    }

    #[test]
    fn prune_events_carry_action_state_reason_and_disk_bytes() {
        let events = prune_events(&pruned());
        let pairs: Vec<(&str, &str)> = events
            .iter()
            .map(|event| {
                (
                    event["action"].as_str().expect("action"),
                    event["state"].as_str().expect("state"),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("prune", "free"),
                ("prune", "skipped"),
                ("prune", "external"),
                ("prune", "kept"),
                ("prune", "kept"),
                ("prune", "kept"),
                ("prune_base", "kept"),
                ("prune_base", "skipped"),
                ("prune_base", "free"),
                ("prune_base", "orphaned"),
                ("prune_base", "nested"),
                ("prune_downloads", "stale"),
            ]
        );
        assert_eq!(events[11]["entries"], 2);
        let expected: [(&str, Option<u64>, Option<&str>, Option<u64>, Option<&str>); 12] = [
            ("/cache/slots/3", Some(3), None, Some(100), None),
            ("/cache/slots/4", Some(4), None, None, Some("busy")),
            ("/ext", None, Some("e"), Some(200), None),
            ("/agent", None, None, None, Some("recent")),
            ("/open", None, Some("o"), None, Some("unmerged")),
            ("/kept", None, Some("k"), None, Some("server")),
            ("/bazel/kkk", None, None, None, Some("locked")),
            ("/bazel/sss", Some(2), None, None, Some("server")),
            ("/bazel/bbb", Some(2), None, Some(1024), None),
            ("/bazel/aaa", None, None, Some(2048), None),
            ("/tmp/dxnb-1000/0/u/nnn", None, None, Some(512), None),
            (
                "/bazel/cache/repos/v1/content_addressable",
                None,
                None,
                Some(1024),
                None,
            ),
        ];
        for (event, (path, slot, branch, bytes, reason)) in events.iter().zip(expected) {
            let object = event.as_object().expect("object");
            assert_eq!(event["event"], "slot", "{event}");
            assert_eq!(event["path"], path, "{event}");
            assert_eq!(object.get("slot").and_then(|v| v.as_u64()), slot, "{event}");
            assert_eq!(
                object.get("branch").and_then(|v| v.as_str()),
                branch,
                "{event}"
            );
            assert_eq!(
                object.get("disk_bytes").and_then(|v| v.as_u64()),
                bytes,
                "{event}"
            );
            assert_eq!(
                object.get("reason").and_then(|v| v.as_str()),
                reason,
                "{event}"
            );
            assert_eq!(object.contains_key("slot"), slot.is_some(), "{event}");
            assert_eq!(object.contains_key("branch"), branch.is_some(), "{event}");
            assert_eq!(
                object.contains_key("disk_bytes"),
                bytes.is_some(),
                "{event}"
            );
            assert_eq!(object.contains_key("reason"), reason.is_some(), "{event}");
        }
    }

    fn run_prune_output(words: &[&str], pruned: &ops::Pruned) -> (i32, String) {
        let mut out = Vec::new();
        let code = prune_output(&mut out, &parsed(words), pruned).expect("written");
        (code, String::from_utf8(out).expect("utf8"))
    }

    #[test]
    fn prune_output_exit_follows_apply_in_text_and_json() {
        let full = pruned();
        for (flags, code) in [
            (&[][..], 1),
            (&["--apply"], 0),
            (&["--output=json"], 1),
            (&["--output=json", "--apply"], 0),
        ] {
            let words: Vec<&str> = ["worktree", "prune"].iter().chain(flags).copied().collect();
            assert_eq!(run_prune_output(&words, &full).0, code, "{flags:?}");
        }
        let (code, text) = run_prune_output(&["worktree", "prune"], &full);
        assert_eq!(code, 1);
        assert_eq!(
            text,
            "kept output base of removed /moved (locked): /bazel/kkk\n\
             kept external worktree (detached) (recent): /agent\n\
             kept external worktree o (unmerged): /open\n\
             skipped slot 4 (busy): /cache/slots/4\n\
             skipped output base of free slot 2 (server): /bazel/sss\n\
             kept external worktree k (server): /kept\n\
             slot 3 idle 40 days, 100 B: /cache/slots/3\n\
             external worktree e, 200 B: /ext\n\
             output base of free slot 2, 1.0 KiB: /bazel/bbb\n\
             output base of removed /gone, 2.0 KiB: /bazel/aaa\n\
             output base of nested Bazel test hello, 512 B: /tmp/dxnb-1000/0/u/nnn\n\
             repository cache downloads unused for 30 days, 2 entries, 1.0 KiB: /bazel/cache/repos/v1/content_addressable\n\
             dx worktree: 4.8 KiB to reclaim; run `dx worktree prune --apply`\n"
        );
        let (code, text) = run_prune_output(&["worktree", "prune", "--apply"], &full);
        assert_eq!(code, 0);
        assert!(text.ends_with("dx worktree: pruned 4.8 KiB\n"), "{text}");
        for words in [
            &["worktree", "prune"][..],
            &["worktree", "prune", "--apply"],
        ] {
            let (code, text) = run_prune_output(words, &ops::Pruned::default());
            assert_eq!(code, 0, "{words:?}");
            assert_eq!(text, "dx worktree: nothing to prune\n", "{words:?}");
        }
        for (words, mode) in [
            (&["worktree", "prune", "--output=json"][..], "check"),
            (
                &["worktree", "prune", "--output=json", "--apply"],
                "default",
            ),
        ] {
            let (_, text) = run_prune_output(words, &full);
            let lines: Vec<serde_json::Value> = text
                .lines()
                .map(|line| serde_json::from_str(line).expect("json line"))
                .collect();
            assert_eq!(lines[0]["event"], "command_started", "{text}");
            assert_eq!(lines[0]["command"], "worktree", "{text}");
            assert_eq!(lines[0]["mode"], mode, "{text}");
            assert_eq!(lines[1..], prune_events(&full)[..], "{text}");
        }
    }

    #[test]
    fn prune_output_prints_kept_externals_with_nothing_to_prune() {
        let only_kept = ops::Pruned {
            plan: PrunePlan {
                kept_externals: pruned().plan.kept_externals,
                ..PrunePlan::default()
            },
            ..ops::Pruned::default()
        };
        for words in [
            &["worktree", "prune"][..],
            &["worktree", "prune", "--apply"],
        ] {
            let (code, text) = run_prune_output(words, &only_kept);
            assert_eq!(code, 0, "{words:?}");
            assert_eq!(
                text,
                "kept external worktree (detached) (recent): /agent\n\
                 kept external worktree o (unmerged): /open\n\
                 dx worktree: nothing to prune\n",
                "{words:?}"
            );
        }
    }

    fn who() -> Who {
        Who::from_env(&|name: &str| (name == "CLAUDE_CODE_SESSION_ID").then(|| "s1".to_owned()))
    }

    fn hook(kind: HookKind, stdin: &str, workspace: &Path) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run_hook(
            kind,
            stdin,
            workspace,
            &SystemHost,
            &who(),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    #[test]
    fn hook_kind_matches_only_the_bare_hook_argv() {
        for kind in HookKind::ALL {
            assert_eq!(hook_kind(&["worktree", "hook", kind.word()]), Some(kind));
        }
        assert_eq!(hook_kind(&["worktree", "hook", "bogus"]), None);
        assert_eq!(hook_kind(&["worktree", "hook", "guard", "x"]), None);
        assert_eq!(hook_kind(&["--verbose", "worktree", "hook", "guard"]), None);
        assert_eq!(hook_kind(&["worktree", "add", "x"]), None);
        assert_eq!(hook_kind(&["worktree", "hook"]), None);
    }

    #[test]
    fn every_generated_hook_command_parses_back_to_its_kind() {
        const LAUNCHER: &str = "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx ";
        for file in dx_worktree::setup::HOOK_FILES {
            let settings: serde_json::Value =
                serde_json::from_str(&dx_worktree::setup::settings_text(file))
                    .expect("settings json");
            let mut kinds: Vec<HookKind> = settings["hooks"]
                .as_object()
                .expect("hooks object")
                .values()
                .flat_map(|groups| groups.as_array().into_iter().flatten())
                .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
                .filter_map(|hook| hook["command"].as_str())
                .map(|command| {
                    let argv: Vec<&str> = command
                        .strip_prefix(LAUNCHER)
                        .expect("launcher prefix")
                        .split_whitespace()
                        .collect();
                    hook_kind(&argv).unwrap_or_else(|| panic!("{command} is not a hook argv"))
                })
                .collect();
            kinds.sort_by_key(|kind| kind.word());
            let mut want: Vec<HookKind> = file.hooks.iter().map(|(kind, _, _)| *kind).collect();
            want.sort_by_key(|kind| kind.word());
            assert_eq!(kinds, want, "{}", file.path);
        }
    }

    #[test]
    fn guard_lets_unreadable_input_through_with_a_note() {
        let dir = tempfile::tempdir().expect("tempdir");
        for stdin in ["", "not json"] {
            let (code, out, err) = hook(HookKind::Guard, stdin, dir.path());
            assert_eq!(code, 0, "{stdin:?}: {err}");
            assert_eq!(out, "");
            assert!(err.contains("command not checked"), "{err}");
        }
    }

    #[test]
    fn guard_blocks_only_git_worktree_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let blocked = r#"{"tool_input":{"command":"git worktree add ../x"}}"#;
        let (code, _, err) = hook(HookKind::Guard, blocked, dir.path());
        assert_eq!(code, 2, "{err}");
        assert!(!err.is_empty());
        let (code, out, err) = hook(
            HookKind::Guard,
            r#"{"tool_input":{"command":"ls"}}"#,
            dir.path(),
        );
        assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    }

    #[test]
    fn gemini_and_cursor_hooks_exit_with_the_codes_they_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gemini = r#"{"session_id":"m1","hook_event_name":"BeforeTool","tool_name":"run_shell_command","tool_input":{"command":"git worktree add ../x"}}"#;
        let (code, out, err) = hook(HookKind::Guard, gemini, dir.path());
        assert_eq!((code, out.as_str()), (2, ""), "{err}");
        assert!(err.contains("dx worktree add"), "{err}");
        let cursor = r#"{"conversation_id":"k1","cursor_version":"2.0","hook_event_name":"subagentStop","subagent_type":"explore","status":"completed"}"#;
        let (code, out, err) = hook(HookKind::Stop, cursor, dir.path());
        assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    }

    #[test]
    fn pool_hooks_fail_with_exit_1() {
        let dir = tempfile::tempdir().expect("tempdir");
        let named = serde_json::json!({ "name": "x", "cwd": dir.path() }).to_string();
        for (kind, stdin) in [
            (HookKind::Create, ""),
            (HookKind::Create, "not json"),
            (HookKind::Create, named.as_str()),
            (HookKind::Remove, "{}"),
            (HookKind::Remove, ""),
            (HookKind::Stop, "{}"),
            (HookKind::Stop, ""),
        ] {
            let (code, out, err) = hook(kind, stdin, dir.path());
            assert_eq!(code, 1, "{kind:?} {stdin:?}: {err}");
            assert_eq!(out, "", "{kind:?} {stdin:?}");
            assert!(err.starts_with("dx worktree: "), "{err}");
            if matches!(stdin, "" | "not json") {
                assert!(err.contains("on stdin"), "{err}");
            }
        }
    }

    #[test]
    fn hook_names_list_every_kind() {
        assert_eq!(HookKind::names(), "create, remove, stop, guard");
        for kind in HookKind::ALL {
            assert_eq!(HookKind::from_word(kind.word()), Some(kind));
        }
        assert_eq!(HookKind::from_word("Guard"), None);
    }
}
