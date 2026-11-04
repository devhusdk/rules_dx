use std::io::Write;
use std::path::Path;

use crate::exec::common::{check_stdout_write, emit_built, emit_event, emit_started};
use dx_output::{command_finished, command_started, status_event, FinishedCounts, StatusEvent};
use dx_plan::args::Invocation;
use dx_plan::resolve::QueryRunner;
use dx_process::Runner;

use dx_output::OutputMode;

use super::status::CODE_STATUS_PIN_MISMATCH;
use super::{init_files, operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";

fn json_pin(
    invocation: &Invocation,
    out: &mut dyn Write,
    mode: &str,
    detail: &str,
    notice: Option<&str>,
) -> Result<(), i32> {
    emit_built(out, command_started(invocation.command.name(), mode))?;
    if let Some(message) = notice {
        emit_built(out, crate::exec::ide_refresh::warning_event(message))?;
    }
    emit_built(
        out,
        status_event(&StatusEvent {
            name: "pin".to_owned(),
            status: "ok".to_owned(),
            detail: detail.to_owned(),
            hint: "dx status".to_owned(),
        }),
    )?;
    emit_event(out, &command_finished(0, &FinishedCounts::default()))
}

/// Reports one failure; JSON output opens the stream first so the envelope stays whole.
fn fail(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    code: &str,
    message: &str,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    }
    operational(invocation, out, err, code, message)
}

const LOCKFILE: &str = "MODULE.bazel.lock";

fn planned_lines(edit: &dx_adopt::ModuleEdit, to: &str) -> Vec<String> {
    vec![
        format!(
            "{}: rules_dx {} -> {to}",
            edit.path,
            current_version(&edit.old)
        ),
        format!("then: {REFRESH}"),
    ]
}

const REFRESH: &str = "bazel mod deps --lockfile_mode=update";

/// Finds the version in the old text of the rules_dx `bazel_dep` file.
fn current_version(text: &str) -> String {
    dx_module_bazel::ModuleFile::parse(text)
        .ok()
        .and_then(|file| {
            file.calls()
                .iter()
                .find(|call| call.name == "bazel_dep" && call.str("name") == Some("rules_dx"))
                .and_then(|call| call.str("version").map(str::to_owned))
        })
        .unwrap_or_else(|| "?".to_owned())
}

/// Writes the edit, then refreshes the lockfile; puts both files back if the refresh fails.
fn write_module(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    edit: &dx_adopt::ModuleEdit,
) -> Result<(), String> {
    let lock = workspace.join(LOCKFILE);
    let lock_before = std::fs::read(&lock).ok();
    dx_adopt::write_module_edit(workspace, edit).map_err(|error| error.to_string())?;
    let Some(lock_bytes) = lock_before else {
        return Ok(());
    };
    let mut argv = invocation.bazel_head().argv("mod");
    argv.extend(["deps".to_owned(), "--lockfile_mode=update".to_owned()]);
    let detail = match runner.run(&argv, workspace, &[]) {
        Ok(status) if status.code == Some(0) => return Ok(()),
        Ok(status) => match status.code {
            Some(code) => format!("exit {code}"),
            None => "killed by a signal".to_owned(),
        },
        Err(error) => error.to_string(),
    };
    let _ = std::fs::write(workspace.join(&edit.path), &edit.old);
    let _ = std::fs::write(&lock, lock_bytes);
    Err(format!(
        "refreshing {LOCKFILE} failed ({detail}); {} restored",
        edit.path
    ))
}

/// Runs the `--to` binary through Bazel so it writes its own pin, launchers and AGENTS block.
fn hand_off(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    to: &str,
    wrote: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let is_json = invocation.output == OutputMode::Json;
    if !is_json && !summaries_suppressed(invocation) {
        for rel in wrote {
            if let Err(exit) = check_stdout_write(writeln!(out, "wrote {rel}")) {
                return exit;
            }
        }
    }
    let mut argv = invocation.bazel_head().argv("run");
    argv.extend(["@rules_dx//:dx", "--", "upgrade", "--to", to, "--apply"].map(str::to_owned));
    if is_json {
        argv.push("--output=json".to_owned());
    }
    match runner.run(&argv, workspace, &[]) {
        Ok(status) => status.code.unwrap_or(1),
        Err(error) => {
            let message = format!(
                "upgrade refused: cannot start rules_dx {to} ({error}); rules_dx is at {to}; finish with bazel run @rules_dx//:dx -- upgrade --to {to} --apply"
            );
            fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
        }
    }
}

/// Moves the `.dx/version` pin to `--to`, newer or older, then refreshes existing
/// launchers and an existing AGENTS.md block. Every refusal runs before the first write.
pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn Runner,
    query_runner: Option<&dyn QueryRunner>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Some(to) = invocation.to.as_deref() else {
        return pre_exec(invocation.output, out, err, "upgrade needs --to <version>");
    };
    let intent = dx_adopt::UpgradeIntent {
        apply: invocation.applies(),
        workspace: invocation.workspace.clone(),
    };
    let refuse = |out: &mut dyn Write, err: &mut dyn Write, message: &str| {
        fail(
            invocation,
            out,
            err,
            CODE_UPGRADE_FAILED,
            &format!("upgrade refused: {message} (nothing was written)"),
        )
    };
    let pin = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) if !pin.is_empty() => pin,
        Ok(_) => return refuse(out, err, ".dx/version is empty; run `dx init --apply`"),
        Err(_) if !workspace.join(".dx/version").is_file() => {
            return refuse(out, err, "no .dx/version; run `dx init --apply`")
        }
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let rerun = dx_adopt::version_pin_matches_module(&pin, to);
    let is_json = invocation.output == OutputMode::Json;
    let edit = match dx_adopt::plan_rules_dx_version(workspace, to) {
        Ok(edit) => edit,
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let edits_module = edit.old != edit.new;
    if !invocation.applies() {
        if rerun && !edits_module {
            if is_json {
                return json_pin(invocation, out, "check", to, None)
                    .map_or_else(|exit| exit, |()| 0);
            }
            if !summaries_suppressed(invocation) {
                if let Err(exit) = check_stdout_write(writeln!(out, "upgrade current: {to}")) {
                    return exit;
                }
            }
            return 0;
        }
        let retry = dx_adopt::upgrade_retry_command(
            to,
            &dx_adopt::UpgradeIntent {
                apply: true,
                ..intent
            },
        );
        let mut message = if rerun {
            format!("rules_dx in {} can move to {to} (run `{retry}`)", edit.path)
        } else {
            format!("pin {pin} can move to {to} (run `{retry}`)")
        };
        if edits_module {
            let plan = planned_lines(&edit, to);
            if is_json {
                message = format!("{message}; {}", plan.join("; "));
            } else if let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.join("\n"))) {
                return exit;
            }
        }
        return fail(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
    }
    if let Err(error) = dx_adopt::plan_launcher_files(workspace, false) {
        return refuse(out, err, &error.to_string());
    }
    let files = match init_files::plan(workspace, "refresh", false) {
        Ok(files) => files,
        Err(error) => return refuse(out, err, &error.to_string()),
    };
    let recovery = dx_adopt::upgrade_recovery_message(to, &intent);
    let landed = |out: &mut dyn Write, err: &mut dyn Write, what: &str| {
        let message = format!("pinned {to} but {what}; {recovery}");
        fail(invocation, out, err, CODE_UPGRADE_FAILED, &message)
    };
    let mut wrote = Vec::new();
    if edits_module {
        if let Err(error) = write_module(invocation, workspace, runner, &edit) {
            return refuse(out, err, &error);
        }
        wrote.push(edit.path.clone());
        if workspace.join(LOCKFILE).is_file() {
            wrote.push(LOCKFILE.to_owned());
        }
    }
    if !dx_adopt::version_pin_matches_module(to, dx_adopt::MODULE_VERSION) {
        return hand_off(invocation, workspace, runner, to, &wrote, out, err);
    }
    if !rerun {
        if let Err(error) = dx_adopt::write_version_pin(workspace, to) {
            return refuse(out, err, &error.to_string());
        }
    }
    let mut applied = dx_adopt::Applied::default();
    if let Err(what) = init_files::apply(
        workspace,
        "",
        &files,
        init_files::Mode::Refresh,
        &mut applied,
        &mut Vec::new(),
    ) {
        return landed(out, err, &what);
    }
    wrote.extend(applied.wrote);
    let ide_warning = query_runner.and_then(|runner| {
        crate::exec::ide_refresh::refresh(workspace, runner, &invocation.bazel_startup_options)
    });
    if is_json {
        return json_pin(invocation, out, "default", to, ide_warning.as_deref())
            .map_or_else(|exit| exit, |()| 0);
    }
    if let Some(message) = &ide_warning {
        let _ = writeln!(
            err,
            "dx: {}: {message}",
            crate::exec::notice_codes::CODE_IDE_REFRESH_FAILED
        );
    }
    let line = if rerun {
        format!("pinned {to} (upgrade current)")
    } else {
        format!("pinned {to} (upgrade from {pin})")
    };
    if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
        return exit;
    }
    if !summaries_suppressed(invocation) {
        for rel in wrote {
            if let Err(exit) = check_stdout_write(writeln!(out, "wrote {rel}")) {
                return exit;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{
        event, event_kinds, invocation, json_events, run, run_with, Recorder,
    };
    use dx_plan::skew::{disposition, is_skewed, SkewDisposition};
    use std::path::PathBuf;

    const OLD: &str = "0.0.0-alpha.1";
    const STALE_AGENTS: &str = "# Mine\n<!-- dx:begin -->\n- old\n<!-- dx:end -->\nTail.\n";

    fn module() -> &'static str {
        dx_adopt::MODULE_VERSION
    }

    fn pinned(prefix: &str, pin: &str) -> (impl Sized, PathBuf) {
        let scratch = dx_test_scratch::scratch(prefix);
        let root = scratch.path().to_path_buf();
        dx_adopt::write_version_pin(&root, pin).expect("pin");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        std::fs::write(root.join("MODULE.bazel.lock"), "{}\n").expect("lock");
        (scratch, root)
    }

    fn dep(version: &str) -> String {
        format!("bazel_dep(name = \"rules_dx\", version = \"{version}\")\n")
    }

    fn module_text(root: &Path) -> String {
        std::fs::read_to_string(root.join("MODULE.bazel")).expect("module")
    }

    fn legacy_launcher(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("tools")).expect("tools");
        let text = format!("#!/bin/sh\nset -eu\nVERSION=\"{version}\"\n");
        std::fs::write(root.join("tools/dx"), text).expect("legacy launcher");
    }

    fn read_pin(root: &Path) -> String {
        dx_adopt::read_version_pin(root).expect("pin")
    }

    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push((path.clone(), std::fs::read(&path).expect("file")));
                }
            }
        }
        files.sort();
        files
    }

    fn upgrade(extra: &[&str]) -> Invocation {
        let to = format!("--to={}", module());
        let mut words = vec!["upgrade", to.as_str()];
        words.extend_from_slice(extra);
        invocation(&words)
    }

    #[test]
    fn the_skew_hint_upgrades_the_pin_and_refreshes_existing_files() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-hint-", OLD);
        assert!(is_skewed(&read_pin(&root)));
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        legacy_launcher(&root, OLD);
        let hint = dx_adopt::pin_fix_hint(OLD, module());
        let words: Vec<&str> = hint.split_whitespace().skip(1).collect();
        assert_eq!(words[0], "upgrade", "{hint}");
        let inv = invocation(&words);
        assert_eq!(disposition(&inv, true), SkewDisposition::Proceed);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!(
                "pinned {} (upgrade from {OLD})\nwrote tools/dx\nwrote AGENTS.md\n",
                module()
            )
        );
        assert_eq!(read_pin(&root), module());
        assert!(!is_skewed(&read_pin(&root)));
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).expect("agents");
        assert_ne!(agents, STALE_AGENTS);
        assert!(agents.starts_with("# Mine\n") && agents.ends_with("Tail.\n"));
        assert_eq!(
            dx_adopt::launcher_state(&root, "tools/dx"),
            Some(dx_adopt::LauncherState::Current)
        );
        assert!(
            !root.join("tools/dx.cmd").exists(),
            "absent launcher created"
        );
    }

    #[test]
    fn upgrade_creates_no_launcher_or_agents_md() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-absent-", OLD);
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!root.join("tools").exists());
        assert!(!root.join("AGENTS.md").exists());
        assert!(!root.join(".vscode").exists());
        assert!(!root.join(".github").exists());
    }

    #[test]
    fn upgrade_rewrites_an_old_pin_in_an_existing_ci_file() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-ci-", OLD);
        let ci = root.join(".github/workflows/ci.yml");
        std::fs::create_dir_all(ci.parent().expect("parent")).expect("dir");
        let fresh = dx_adopt::render_consumer_caller();
        let stale = fresh.replace(dx_adopt::DX_VERSION, "0.0.1");
        std::fs::write(&ci, &stale).expect("stale");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote .github/workflows/ci.yml"), "{out}");
        assert_eq!(std::fs::read_to_string(&ci).expect("read"), fresh);
    }

    #[test]
    fn upgrade_refreshes_only_what_exists_from_the_init_plan() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-plan-", OLD);
        let plan = init_files::plan(&root, "refresh", false).expect("plan");
        let mut applied = dx_adopt::Applied::default();
        init_files::apply(
            &root,
            "",
            &plan,
            init_files::Mode::Create,
            &mut applied,
            &mut Vec::new(),
        )
        .expect("init");
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        dx_adopt::write_version_pin(&root, OLD).expect("pin");
        legacy_launcher(&root, OLD);
        let ci = root.join(".github/workflows/ci.yml");
        let fresh = dx_adopt::render_consumer_caller();
        std::fs::write(&ci, fresh.replace(dx_adopt::DX_VERSION, "0.0.1")).expect("stale");
        let mut gone = vec![
            "tools/dx.cmd".to_owned(),
            ".vscode/settings.json".to_owned(),
            "AGENTS.md".to_owned(),
            "CLAUDE.md".to_owned(),
            dx_worktree::setup::HOOK_FILES[0].path.to_owned(),
            dx_adopt::render_overrides().remove(0).0,
        ];
        for path in &gone {
            std::fs::remove_file(root.join(path)).expect("remove");
        }
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        let known: Vec<&str> = plan.iter().map(|file| file.path.as_str()).collect();
        for line in out.lines().filter_map(|line| line.strip_prefix("wrote ")) {
            assert!(known.contains(&line), "{line} is not in the plan");
        }
        assert!(out.contains("wrote tools/dx\n"), "{out}");
        assert!(out.contains("wrote .github/workflows/ci.yml"), "{out}");
        gone.retain(|path| root.join(path).exists());
        assert!(gone.is_empty(), "recreated {gone:?}");
    }

    #[test]
    fn upgrade_runs_on_skew() {
        assert_eq!(
            disposition(&upgrade(&["--apply"]), true),
            SkewDisposition::Proceed
        );
    }

    #[test]
    fn check_mode_writes_nothing_and_reports_pending_or_current() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-check-", OLD);
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("agents");
        let before = snapshot(&root);
        let (code, _out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_STATUS_PIN_MISMATCH), "{err}");
        assert!(err.contains(&format!("can move to {}", module())), "{err}");
        assert!(err.contains("--apply`"), "{err}");
        assert_eq!(snapshot(&root), before, "check must not write");
        let (_scratch, root) = pinned("dx-adopt-upgrade-current-", module());
        let (code, out, err) = run(&upgrade(&[]), &root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(out, format!("upgrade current: {}\n", module()));
    }

    #[test]
    fn upgrade_moves_a_newer_pin_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-down-", "9.9.9");
        dx_adopt::write_launcher_files(&root, &mut dx_adopt::Applied::default())
            .expect("launchers");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.starts_with(&format!("pinned {} (upgrade from 9.9.9)\n", module())),
            "{out}"
        );
        assert_eq!(read_pin(&root), module());
    }

    #[test]
    fn upgrade_changes_only_the_pin_file_once_launchers_are_static() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-static-", OLD);
        dx_adopt::write_launcher_files(&root, &mut dx_adopt::Applied::default())
            .expect("launchers");
        let before = snapshot(&root);
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("wrote tools/dx"), "{out}");
        let changed: Vec<PathBuf> = snapshot(&root)
            .into_iter()
            .filter(|entry| !before.contains(entry))
            .map(|(path, _)| path)
            .collect();
        assert_eq!(changed, vec![root.join(".dx/version")]);
    }

    #[test]
    fn upgrade_moves_forward_and_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-round-", OLD);
        assert_eq!(run(&upgrade(&["--apply"]), &root).0, 0);
        assert_eq!(read_pin(&root), module());
        dx_adopt::write_version_pin(&root, "9.9.9").expect("newer pin");
        let (code, out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(read_pin(&root), module());
    }

    #[test]
    fn a_missing_pin_points_at_init() {
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-nopin-");
        let root = scratch.path().to_path_buf();
        let (code, _out, err) = run(&upgrade(&["--apply"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("dx init --apply"), "{err}");
        assert!(!root.join(".dx").exists());
    }

    #[test]
    fn another_version_edits_the_dep_refreshes_and_hands_off() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-to-", "1.2.3");
        let runner = Recorder::default();
        let inv = invocation(&["upgrade", "--apply", "--to=2.0.0"]);
        let (code, out, err) = run_with(&inv, &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep("2.0.0"));
        assert_eq!(out, "wrote MODULE.bazel\nwrote MODULE.bazel.lock\n");
        assert_eq!(read_pin(&root), "1.2.3");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].contains(&"mod".to_owned()), "{calls:?}");
        assert!(calls[0].ends_with(&["deps".to_owned(), "--lockfile_mode=update".to_owned()]));
        assert!(calls[1].contains(&"run".to_owned()), "{calls:?}");
        assert!(calls[1].ends_with(
            &[
                "@rules_dx//:dx",
                "--",
                "upgrade",
                "--to",
                "2.0.0",
                "--apply"
            ]
            .map(str::to_owned)
        ));
    }

    #[test]
    fn a_json_handoff_writes_nothing_first_and_returns_its_code() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-hand-json-", "1.2.3");
        let runner = Recorder::scripted(&[0, 7]);
        let inv = invocation(&["upgrade", "--apply", "--to=2.0.0", "--output=json"]);
        let (code, out, _err) = run_with(&inv, &root, &runner);
        assert_eq!(code, 7);
        assert_eq!(out, "");
        let calls = runner.calls.borrow();
        assert_eq!(calls[1].last().map(String::as_str), Some("--output=json"));
    }

    #[test]
    fn this_dx_with_an_older_dep_edits_refreshes_and_moves_the_pin() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-older-dep-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep("0.0.1")).expect("module");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep(module()));
        assert_eq!(read_pin(&root), module());
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(out.contains("wrote MODULE.bazel.lock"), "{out}");
    }

    #[test]
    fn a_failed_refresh_puts_every_file_back() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-refresh-fails-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        let before = snapshot(&root);
        let runner = Recorder::scripted(&[1]);
        let (code, _out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains("refreshing MODULE.bazel.lock failed (exit 1)"),
            "{err}"
        );
        assert_eq!(snapshot(&root), before);
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn a_dep_in_an_included_file_is_edited_there() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-include-", OLD);
        std::fs::write(
            root.join("MODULE.bazel"),
            "include(\"//:deps.MODULE.bazel\")\n",
        )
        .expect("module");
        std::fs::write(root.join("deps.MODULE.bazel"), dep(OLD)).expect("include");
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &Recorder::default());
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote deps.MODULE.bazel\n"), "{out}");
        let text = std::fs::read_to_string(root.join("deps.MODULE.bazel")).expect("include");
        assert_eq!(text, dep(module()));
    }

    #[test]
    fn no_rules_dx_dep_is_refused_without_a_runner_call() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-no-dep-", OLD);
        std::fs::write(root.join("MODULE.bazel"), "module(name = \"x\")\n").expect("module");
        let before = snapshot(&root);
        let runner = Recorder::default();
        let (code, _out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(err.contains("bazel_dep"), "{err}");
        assert_eq!(snapshot(&root), before);
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn check_mode_names_the_planned_module_change_and_runs_nothing() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-plan-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        let before = snapshot(&root);
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&[]), &root, &runner);
        assert_eq!(code, 1, "{err}");
        assert_eq!(
            out,
            format!(
                "MODULE.bazel: rules_dx {OLD} -> {}\nthen: bazel mod deps --lockfile_mode=update\n",
                module()
            )
        );
        assert_eq!(snapshot(&root), before);
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn no_lockfile_or_a_current_dep_skips_the_refresh() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-no-lock-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(OLD)).expect("module");
        std::fs::remove_file(root.join("MODULE.bazel.lock")).expect("lock");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(module_text(&root), dep(module()));
        assert!(runner.calls.borrow().is_empty());
        let (_scratch, root) = pinned("dx-adopt-upgrade-dep-current-", OLD);
        std::fs::write(root.join("MODULE.bazel"), dep(module())).expect("module");
        let runner = Recorder::default();
        let (code, out, err) = run_with(&upgrade(&["--apply"]), &root, &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn a_rerun_after_apply_succeeds() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-rerun-", OLD);
        let apply = upgrade(&["--apply"]);
        assert_eq!(run(&apply, &root).0, 0);
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, format!("pinned {} (upgrade current)\n", module()));
    }

    #[test]
    fn upgrade_json_streams_the_status_envelope() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-", OLD);
        let (code, out, _err) = run(&upgrade(&["--apply", "--output=json"]), &root);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "status", "command_finished"]
        );
        assert_eq!(events[1]["name"], "pin");
        let (_scratch, root) = pinned("dx-adopt-upgrade-json-refused-", OLD);
        std::fs::remove_file(root.join("MODULE.bazel")).expect("module");
        let inv = upgrade(&["--apply", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            vec!["command_started", "error", "command_finished"]
        );
        assert_eq!(event(&events, "error")["code"], CODE_UPGRADE_FAILED);
    }

    #[test]
    fn following_the_hint_for_a_newer_pin_clears_skew() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-newer-", "9.9.9");
        let hint = dx_adopt::pin_fix_hint("9.9.9", module());
        let words: Vec<&str> = hint.split_whitespace().skip(1).collect();
        let (code, out, err) = run(&invocation(&words), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!is_skewed(&read_pin(&root)));
    }

    #[test]
    fn an_orphaned_agents_marker_after_the_pin_lands_names_recovery() {
        let (_scratch, root) = pinned("dx-adopt-upgrade-orphan-", OLD);
        std::fs::write(root.join("AGENTS.md"), "# Mine\n<!-- dx:begin -->\n").expect("agents");
        let apply = upgrade(&["--apply"]);
        let (code, _out, err) = run(&apply, &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_UPGRADE_FAILED), "{err}");
        assert!(
            err.contains(&format!("pinned {} but AGENTS.md not refreshed", module())),
            "{err}"
        );
        assert!(err.contains("recovery: rerun"), "{err}");
        assert_eq!(read_pin(&root), module());
        std::fs::write(root.join("AGENTS.md"), STALE_AGENTS).expect("fix");
        let (code, out, err) = run(&apply, &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("wrote AGENTS.md"), "{out}");
    }

    #[test]
    fn upgrade_codes_are_stable_single_source() {
        assert_eq!(CODE_UPGRADE_FAILED, "upgrade_failed");
    }
}
