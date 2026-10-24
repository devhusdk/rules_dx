use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::common::*;
use crate::args::Invocation;
use crate::worktree::host::{Host, SystemHost};
use crate::worktree::ops::{self, AddRequest, Added, Listing, PrunePlan, Row};
use crate::worktree::pool::{cache_base, Pool, Who};
use crate::worktree::{hook, HookKind, WorktreeError};
use dx_output::{
    command_finished, command_started, notice_event, slot_event, summary_event, FinishedCounts,
    NoticeEvent, OutputMode, SlotEvent, SummaryEvent,
};

const CODE_WORKTREE_FAILED: &str = "worktree_failed";

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn open_pool(dir: &Path) -> Result<Pool, WorktreeError> {
    let base = cache_base(&env_var).ok_or_else(|| {
        WorktreeError::refused(
            "no_home",
            "cannot find a home directory for the worktree pool",
        )
    })?;
    Pool::open(dir, &base)
}

fn size(bytes: u64) -> String {
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

fn row_event(action: &str, row: &Row) -> serde_json::Value {
    slot_event(&SlotEvent {
        action: action.to_owned(),
        path: row.path.to_string_lossy().into_owned(),
        slot: row.slot,
        branch: row.branch.clone(),
        holder: row.holder.clone(),
        state: row.state.name().to_owned(),
        reason: row.reason.map(str::to_owned),
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
        if let Ok(event) = command_started(invocation.command.name(), mode) {
            emit_event(out, &event)?;
        }
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
    let code = if error.usage {
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
        ["add", branch] => add(invocation, workspace, &host, &who, branch, out),
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
    add_output(out, invocation, branch, &added)?;
    Ok(0)
}

fn add_output(
    out: &mut dyn Write,
    invocation: &Invocation,
    branch: &str,
    added: &Added,
) -> Result<(), i32> {
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        for warning in &added.warnings {
            let notice = NoticeEvent {
                level: "warning".to_owned(),
                code: warning.code.to_owned(),
                message: warning.message.clone(),
                related_command: Some("worktree".to_owned()),
                scope: None,
                path: None,
                language: None,
                import: None,
            };
            if let Ok(event) = notice_event(&notice) {
                emit_event(out, &event)?;
            }
        }
        emit_event(
            out,
            &simple_event(
                "add",
                Some(added.slot),
                &added.path,
                Some(branch.to_owned()),
                "held",
            ),
        )?;
    } else if invocation.path_only {
        write_line(out, &added.path.to_string_lossy())?;
    } else {
        for warning in &added.warnings {
            write_line(out, &format!("dx worktree: {}", warning.message))?;
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

fn release(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    who: &Who,
    path: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let target = path
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());
    let pool = open_pool(workspace)?;
    ops::claim(&pool, who, workspace)?;
    let released = ops::release(&pool, host, who, &target, invocation.force)?;
    start(out, invocation, "default")?;
    if invocation.output == OutputMode::Json {
        emit_event(
            out,
            &simple_event(
                "release",
                released.slot,
                &released.path,
                released.branch,
                if released.removed { "removed" } else { "free" },
            ),
        )?;
    } else if released.removed {
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
    Ok(0)
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
                disk_cache_bytes: listing.disk_cache_bytes,
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
        lines.push(format!(
            "output base of removed {}, {}: {}",
            base.workspace.display(),
            size(base.bytes),
            base.dir.display()
        ));
    }
    lines
}

fn prune(
    invocation: &Invocation,
    workspace: &Path,
    host: &dyn Host,
    out: &mut dyn Write,
) -> Result<i32, Failed> {
    let pool = open_pool(workspace)?;
    let mut plan = ops::prune_plan(&pool, host);
    let apply = invocation.applies();
    let kept = if apply {
        ops::prune_apply(&pool, host, &plan)?
    } else {
        Vec::new()
    };
    plan.externals
        .retain(|external| !kept.iter().any(|item| item.path == external.path));
    start(out, invocation, if apply { "default" } else { "check" })?;
    if invocation.output == OutputMode::Json {
        for slot in &plan.slots {
            let mut event = simple_event("prune", Some(slot.slot), &slot.path, None, "free");
            event["disk_bytes"] = slot.bytes.into();
            emit_event(out, &event)?;
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
            emit_event(out, &event)?;
        }
        for item in &kept {
            let mut event = simple_event("prune", None, &item.path, item.branch.clone(), "kept");
            event["reason"] = item.reason.into();
            emit_event(out, &event)?;
        }
        for base in &plan.bases {
            let mut event = simple_event("prune_base", None, &base.dir, None, "orphaned");
            event["disk_bytes"] = base.bytes.into();
            emit_event(out, &event)?;
        }
    } else if plan.is_empty() && kept.is_empty() {
        write_line(out, "dx worktree: nothing to prune")?;
    } else {
        for item in &kept {
            write_line(
                out,
                &format!(
                    "kept external worktree {} ({}): {}",
                    item.branch.as_deref().unwrap_or("(detached)"),
                    item.reason,
                    item.path.display()
                ),
            )?;
        }
        for line in prune_lines(&plan) {
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
    if apply {
        return Ok(0);
    }
    Ok(if plan.is_empty() { 0 } else { 1 })
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

/// Runs one Claude Code hook without loading dx defaults.
pub fn run_worktree_hook(kind: HookKind, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let stdin = read_stdin();
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let who = Who::from_env(&env_var);
    run_hook(kind, &stdin, &workspace, &SystemHost, &who, out, err)
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
        HookKind::Create => pool_hook(stdin, workspace, err, |pool, input| {
            hook::create(pool, host, who, input).map(|path| {
                match write_line(out, &path.to_string_lossy()) {
                    Ok(()) => 0,
                    Err(exit) => exit,
                }
            })
        }),
        HookKind::Stop => pool_hook(stdin, workspace, err, |pool, input| {
            hook::stop(pool, who, input).map(|_| 0)
        }),
        HookKind::Remove => pool_hook(stdin, workspace, err, |pool, input| {
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
    stdin: &str,
    workspace: &Path,
    err: &mut dyn Write,
    op: impl FnOnce(&Pool, &serde_json::Value) -> Result<i32, WorktreeError>,
) -> i32 {
    let result = hook::parse(stdin).and_then(|input| {
        let dir = hook::cwd(&input).unwrap_or_else(|| workspace.to_path_buf());
        open_pool(&dir).and_then(|pool| op(&pool, &input))
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
    use crate::args::parsed;
    use crate::worktree::ops::Warning;

    fn added() -> Added {
        Added {
            slot: 3,
            path: PathBuf::from("/cache/slots/3"),
            reused: false,
            warnings: vec![Warning {
                code: "fetch_failed",
                message: "cannot fetch origin, using local refs: offline".to_owned(),
            }],
        }
    }

    fn render(words: &[&str]) -> String {
        let invocation = parsed(words);
        let mut out = Vec::new();
        add_output(&mut out, &invocation, "x", &added()).expect("output");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn add_json_emits_a_warning_notice_before_the_slot() {
        let text = render(&["worktree", "add", "x", "--output=json"]);
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
    }

    #[test]
    fn add_text_prints_the_warning_before_the_slot_line() {
        let text = render(&["worktree", "add", "x"]);
        assert_eq!(
            text,
            "dx worktree: cannot fetch origin, using local refs: offline\n\
             dx worktree: x is in slot 3 at /cache/slots/3\n"
        );
    }

    #[test]
    fn add_path_prints_only_the_path() {
        let text = render(&["worktree", "add", "x", "--output=path"]);
        assert_eq!(text, "/cache/slots/3\n");
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
