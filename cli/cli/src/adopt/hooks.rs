use std::io::Write;
use std::time::Instant;

use crate::args::{Command, Invocation};
use crate::exec::common::check_stdout_write;

use crate::resolve::{QueryResult, QueryRunner};

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_HOOKS_FAILED: &str = "hooks_failed";

pub(crate) fn execute_hooks(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    runner: &dyn dx_process::Runner,
    hook_stdin: Option<&[u8]>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let verb = invocation.targets.first().map(String::as_str).unwrap_or("");
    match verb {
        "install" => {
            if invocation.dry_run {
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install .git/hooks/pre-commit"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install .git/hooks/pre-push"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install dx.local.toml"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            if !invocation.applies() {
                return execute_install_check(invocation, workspace, out, err);
            }
            match dx_adopt::install_hooks(workspace) {
                Ok(installed) => {
                    if !summaries_suppressed(invocation) {
                        for path in installed {
                            if let Err(exit) = check_stdout_write(writeln!(out, "installed {path}"))
                            {
                                return exit;
                            }
                        }
                    }
                    0
                }
                Err(error) => {
                    operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string())
                }
            }
        }
        "uninstall" => {
            if invocation.dry_run {
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would remove .git/hooks/pre-commit"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would remove .git/hooks/pre-push"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            if !invocation.applies() {
                return execute_uninstall_check(invocation, workspace, out, err);
            }
            match dx_adopt::uninstall_hooks(workspace) {
                Ok(removed) => {
                    if !summaries_suppressed(invocation) {
                        for path in removed {
                            if let Err(exit) = check_stdout_write(writeln!(out, "removed {path}")) {
                                return exit;
                            }
                        }
                    }
                    0
                }
                Err(error) => {
                    operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string())
                }
            }
        }
        "status" => execute_status(invocation, workspace, out, err),
        "run" => execute_run(
            invocation,
            workspace,
            query_runner,
            runner,
            hook_stdin,
            out,
            err,
        ),
        _ => pre_exec(
            err,
            &format!("usage: dx hooks <{}>", dx_adopt::hook_verb_pipe()),
        ),
    }
}

fn read_file_opt(root: &std::path::Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn execute_install_check(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let drifted = match dx_adopt::check_hooks_install(workspace) {
        Ok(drifted) => drifted,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    if drifted.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "hooks ok: install current")) {
                return exit;
            }
        }
        return 0;
    }
    operational(
        invocation,
        out,
        err,
        CODE_HOOKS_FAILED,
        &format!(
            "hooks check failed: install would write {} (re-run with --apply to install)",
            drifted.join(", "),
        ),
    )
}

fn execute_uninstall_check(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let drifted = match dx_adopt::check_hooks_uninstall(workspace) {
        Ok(drifted) => drifted,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    if drifted.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "hooks ok: uninstall current")) {
                return exit;
            }
        }
        return 0;
    }
    operational(
        invocation,
        out,
        err,
        CODE_HOOKS_FAILED,
        &format!(
            "hooks check failed: uninstall would remove {} (re-run with --apply to remove)",
            drifted.join(", "),
        ),
    )
}

fn execute_status(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "would show hooks status")) {
                return exit;
            }
        }
        return 0;
    }
    let baseline_opt = read_file_opt(workspace, dx_adopt::HOOK_BASELINE_REL);
    let overlay_opt = read_file_opt(workspace, dx_adopt::HOOK_OVERLAY_REL);
    let timings_opt = read_file_opt(workspace, dx_adopt::HOOK_TIMINGS_REL);
    let baseline_res = dx_adopt::load_hooks_config(baseline_opt.as_deref(), None);
    let overlay_res = dx_adopt::load_hooks_config(None, overlay_opt.as_deref());
    let timings_res = dx_adopt::load_hook_timings(timings_opt.as_deref());
    let shows_baseline = baseline_res.is_ok();
    let shows_overlay = overlay_res.is_ok();
    let shows_timings = timings_res.is_ok();
    if !dx_adopt::hook_status_shows_merged(shows_baseline, shows_overlay, shows_timings) {
        let mut detail = String::new();
        if let Err(error) = &baseline_res {
            detail.push_str(&error.to_string());
            detail.push_str("; ");
        }
        if let Err(error) = &overlay_res {
            detail.push_str(&error.to_string());
            detail.push_str("; ");
        }
        if let Err(error) = &timings_res {
            detail.push_str(&error.to_string());
        }
        let detail = detail.trim_end_matches("; ");
        return operational(
            invocation,
            out,
            err,
            CODE_HOOKS_FAILED,
            &format!("hooks status missing merged layer: {detail}"),
        );
    }
    let merged = match dx_adopt::load_hooks_config(baseline_opt.as_deref(), overlay_opt.as_deref())
    {
        Ok(config) => config,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let timings = match timings_res {
        Ok(timings) => timings,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let baseline_src = if baseline_opt.is_some() {
        dx_adopt::HOOK_BASELINE_REL.to_owned()
    } else {
        "defaults (no dx.hooks.toml)".to_owned()
    };
    let overlay_src = if overlay_opt.is_some() {
        dx_adopt::HOOK_OVERLAY_REL.to_owned()
    } else {
        "absent (no dx.local.toml)".to_owned()
    };
    let view = dx_adopt::render_hooks_status_merged(&merged, &timings, &baseline_src, &overlay_src);
    if let Err(exit) = check_stdout_write(write!(out, "{view}")) {
        return exit;
    }
    0
}

fn execute_run(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    runner: &dyn dx_process::Runner,
    hook_stdin: Option<&[u8]>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let trigger = invocation.targets.get(1).map(String::as_str).unwrap_or("");
    if !dx_adopt::is_hook_trigger(trigger) {
        return pre_exec(
            err,
            &format!("usage: dx hooks run <{}>", dx_adopt::hook_trigger_pipe()),
        );
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "would run {trigger}")) {
                return exit;
            }
        }
        return 0;
    }
    let git_path = runner.git_tool();
    let uses_hermetic = git_path
        .as_ref()
        .is_some_and(|path| dx_adopt::hook_git_path_is_hermetic(path));
    if !dx_adopt::hook_git_is_hermetic(uses_hermetic, false) {
        return operational(
            invocation,
            out,
            err,
            CODE_HOOKS_FAILED,
            &format!(
                "hook git must be hermetic: set {} to an absolute managed Git path; ambient PATH lookup is rejected",
                dx_adopt::HOOK_GIT_ENV_VAR
            ),
        );
    }
    let git = git_path.unwrap_or_else(|| std::path::PathBuf::from("git"));
    let baseline_opt = read_file_opt(workspace, dx_adopt::HOOK_BASELINE_REL);
    let overlay_opt = read_file_opt(workspace, dx_adopt::HOOK_OVERLAY_REL);
    let config = match dx_adopt::load_hooks_config(baseline_opt.as_deref(), overlay_opt.as_deref())
    {
        Ok(config) => config,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let checks = dx_adopt::checks_for_trigger(&config, trigger);
    if checks.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "ran {trigger}: ok (no checks configured)"))
            {
                return exit;
            }
        }
        return 0;
    }
    let (changes, source) = match trigger {
        "pre-commit" => match staged_changes(&git, workspace, query_runner) {
            Ok(changes) => (changes, dx_adopt::ChangeSource::Staged),
            Err(detail) => {
                return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
            }
        },
        _ => {
            let stdin_bytes = match read_push_stdin(hook_stdin) {
                Ok(bytes) => bytes,
                Err(detail) => {
                    return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
                }
            };
            let refs = match dx_adopt::parse_push_refs(&stdin_bytes) {
                Ok(refs) => refs,
                Err(detail) => {
                    return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
                }
            };
            if refs.is_empty() {
                if !summaries_suppressed(invocation) {
                    if let Err(exit) = check_stdout_write(writeln!(
                        out,
                        "ran {trigger}: ok (no pushed refs on stdin)"
                    )) {
                        return exit;
                    }
                }
                return 0;
            }
            match pushed_changes(&git, workspace, query_runner, &refs) {
                Ok(changes) => (changes, dx_adopt::ChangeSource::Pushed),
                Err(detail) => {
                    return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
                }
            }
        }
    };
    if changes.is_empty() {
        let note = match source {
            dx_adopt::ChangeSource::Staged => "no staged changes",
            dx_adopt::ChangeSource::Pushed => "no pushed changes",
        };
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "ran {trigger}: ok ({note})")) {
                return exit;
            }
        }
        return 0;
    }
    let targets = match change_targets(
        &changes,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(targets) => targets,
        Err(detail) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
        }
    };
    if !summaries_suppressed(invocation) {
        let line = dx_adopt::render_selection_line(trigger, &source, changes.len(), targets.len());
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    let dx_exe = match std::env::current_exe() {
        Ok(exe) => exe.to_string_lossy().into_owned(),
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_HOOKS_FAILED,
                &format!("hook dx executable unavailable: {error}"),
            );
        }
    };
    let mut measured: Vec<(String, f64)> = Vec::with_capacity(checks.len());
    for check in &checks {
        let argv = check_argv(&dx_exe, check, &targets);
        let start = Instant::now();
        let status = match runner.run(&argv, workspace, &[]) {
            Ok(status) => status,
            Err(error) => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} launch failed: {error}"),
                );
            }
        };
        let elapsed = start.elapsed().as_secs_f64();
        if dx_adopt::hook_check_timed_out(elapsed, config.budget_secs) {
            return operational(
                invocation,
                out,
                err,
                CODE_HOOKS_FAILED,
                &format!(
                    "hook check {check:?} exceeded budget (took {elapsed:.2}s, budget {}s)",
                    config.budget_secs
                ),
            );
        }
        match status.code {
            Some(0) => {
                measured.push((check.clone(), elapsed));
            }
            Some(code) => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} failed with exit {code}"),
                );
            }
            None => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} terminated by signal"),
                );
            }
        }
    }
    if hook_applies(invocation, &checks) {
        if let Err(detail) = record_timings(workspace, &measured) {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
        }
    }
    if !summaries_suppressed(invocation) {
        for (check, elapsed) in &measured {
            let _ = writeln!(
                out,
                "ran {trigger}: {check} ok ({elapsed:.2}s / budget {}s)",
                config.budget_secs
            );
        }
    }
    0
}

fn read_push_stdin(injected: Option<&[u8]>) -> Result<Vec<u8>, String> {
    match injected {
        Some(bytes) => Ok(bytes.to_vec()),
        None => {
            let mut buf = Vec::new();
            use std::io::Read;
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|error| format!("hook pre-push stdin unreadable: {error}"))?; // LCOV_EXCL_LINE - reason: process stdin, issue: 1348, policy: docs/cli/commands/build-test-coverage.md
            Ok(buf)
        }
    }
}

fn run_git_name_status(
    git: &std::path::Path,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    extra: &[String],
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(extra.iter().cloned());
    let result: QueryResult = query_runner
        .run_query(&argv, workspace)
        .map_err(|error| format!("hook git diff failed: {error}"))?;
    if result.code != Some(0) {
        let detail = first_line(&result.stderr);
        return Err(format!("hook git diff failed: {detail}"));
    }
    dx_adopt::parse_name_status_nul(&result.stdout)
}

fn staged_changes(
    git: &std::path::Path,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
) -> Result<Vec<dx_adopt::GitChange>, String> {
    run_git_name_status(
        git,
        workspace,
        query_runner,
        &[
            "diff".to_owned(),
            "--cached".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
        ],
    )
    .map(dx_adopt::dedupe_changes)
}

fn pushed_changes(
    git: &std::path::Path,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    refs: &[dx_adopt::PushRef],
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut changes = Vec::new();
    for push_ref in refs {
        let Some(base) = dx_adopt::push_diff_base(push_ref) else {
            continue;
        };
        changes.extend(run_git_name_status(
            git,
            workspace,
            query_runner,
            &[
                "diff".to_owned(),
                "--name-status".to_owned(),
                "-z".to_owned(),
                base,
                push_ref.local_sha.clone(),
            ],
        )?);
    }
    Ok(dx_adopt::dedupe_changes(changes))
}

fn change_targets(
    changes: &[dx_adopt::GitChange],
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, String> {
    let mut existing = Vec::new();
    let mut missing = Vec::new();
    for change in changes {
        for path in change.from.iter().chain(std::iter::once(&change.path)) {
            if workspace.join(path).is_file() || workspace.join(path).is_dir() {
                existing.push(path.clone());
            } else {
                missing.push(path.clone());
            }
        }
    }
    existing.sort();
    existing.dedup();
    missing.sort();
    missing.dedup();
    let mut targets = if existing.is_empty() {
        Vec::new()
    } else {
        crate::resolve::resolve(&existing, workspace, query_runner, startup_options)
            .map(|resolved| resolved.targets)
            .map_err(|error| error.to_string())?
    };
    for path in &missing {
        if !existing.iter().any(|kept| path == kept) {
            targets.push(dx_adopt::nearest_package_pattern(workspace, path));
        }
    }
    targets.sort();
    targets.dedup();
    Ok(targets)
}

fn check_argv(dx_exe: &str, check: &str, targets: &[String]) -> Vec<String> {
    let mut words: Vec<String> = check.split_whitespace().map(ToOwned::to_owned).collect();
    let explicit_mode = words
        .iter()
        .any(|word| matches!(word.as_str(), "--check" | "--apply" | "--dry-run"));
    if !explicit_mode {
        let forces_check = words
            .first()
            .and_then(|first| Command::parse(first))
            .is_some_and(|command| command.is_mutating_by_default() && command.supports_check());
        if forces_check {
            words.insert(1, "--check".to_owned());
        }
    }
    let mut argv = vec![dx_exe.to_owned()];
    argv.extend(words);
    argv.extend(targets.iter().cloned());
    argv
}

fn hook_applies(invocation: &Invocation, checks: &[String]) -> bool {
    invocation.apply
        || checks
            .iter()
            .any(|check| check.split_whitespace().any(|word| word == "--apply"))
}

fn record_timings(workspace: &std::path::Path, measured: &[(String, f64)]) -> Result<(), String> {
    let path = workspace.join(dx_adopt::HOOK_TIMINGS_REL);
    let existing = std::fs::read_to_string(&path).ok();
    let mut timings =
        dx_adopt::load_hook_timings(existing.as_deref()).map_err(|error| error.to_string())?;
    for (check, elapsed) in measured {
        timings.secs_by_check.insert(check.clone(), *elapsed);
    }
    let body = dx_adopt::render_hook_timings(&timings).map_err(|error| error.to_string())?;
    dx_atomic_fs::write_atomic(&path, body.as_bytes())
        .map_err(|error| format!("write timings: {error}"))?;
    Ok(())
}

fn first_line(bytes: &[u8]) -> String {
    dx_output::first_diagnostic_line(bytes, 200, "no Git diagnostic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{
        invocation, run, run_with, run_with_query, NullQuery, NullRunner,
    };
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

    #[test]
    fn hooks_install_uninstall_and_collisions_are_reported() {
        let scratch = dx_test_scratch::scratch("hooks-install-cycle-");
        let root = scratch.path();
        std::fs::create_dir(root.join(".git")).expect("git");
        for verb in ["install", "uninstall"] {
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &invocation(&["hooks", "--apply", verb]),
                    root,
                    &NullQuery,
                    &NullRunner,
                    None,
                    &mut out,
                    &mut err
                ),
                0
            );
            assert!(String::from_utf8(out)
                .expect("out")
                .contains(if verb == "install" {
                    "installed"
                } else {
                    "removed"
                }));
            assert!(err.is_empty());
        }
        let foreign = dx_test_scratch::scratch("hooks-install-collision-");
        std::fs::create_dir_all(foreign.path().join(".git/hooks")).expect("hooks");
        std::fs::write(foreign.path().join(".git/hooks/pre-commit"), "foreign hook")
            .expect("foreign hook");
        for verb in ["install", "uninstall"] {
            for mode in [&[] as &[&str], &["--apply"]] {
                let mut words = vec!["hooks"];
                words.extend(mode.iter().copied());
                words.push(verb);
                let mut err = Vec::new();
                assert_eq!(
                    execute_hooks(
                        &invocation(&words),
                        foreign.path(),
                        &NullQuery,
                        &NullRunner,
                        None,
                        &mut Vec::new(),
                        &mut err
                    ),
                    1,
                    "{words:?}"
                );
                assert!(!err.is_empty());
            }
        }
    }

    #[test]
    fn hooks_install_check_reports_drift_without_writing() {
        let scratch = dx_test_scratch::scratch("hooks-install-check-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir(root.join(".git")).expect("git");
        let inv = invocation(&["hooks", "install"]);
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_HOOKS_FAILED), "{err}");
        assert!(err.contains(".git/hooks/pre-commit"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert!(!root.join(".git/hooks/pre-commit").exists());
        assert!(!root.join("dx.local.toml").exists());
    }

    #[test]
    fn hooks_install_check_passes_once_applied() {
        let scratch = dx_test_scratch::scratch("hooks-install-check-ok-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir(root.join(".git")).expect("git");
        let (code, _, err) = run(&invocation(&["hooks", "--apply", "install"]), &root);
        assert_eq!(code, 0, "{err}");
        let (code, out, err) = run(&invocation(&["hooks", "install", "--check"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("hooks ok: install current"), "{out}");
    }

    #[test]
    fn hooks_uninstall_check_reports_managed_shims_without_removing() {
        let scratch = dx_test_scratch::scratch("hooks-uninstall-check-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir(root.join(".git")).expect("git");
        let (code, out, err) = run(&invocation(&["hooks", "uninstall"]), &root);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("hooks ok: uninstall current"), "{out}");
        let (code, _, err) = run(&invocation(&["hooks", "--apply", "install"]), &root);
        assert_eq!(code, 0, "{err}");
        let (code, _out, err) = run(&invocation(&["hooks", "uninstall"]), &root);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains(CODE_HOOKS_FAILED), "{err}");
        assert!(err.contains(".git/hooks/pre-commit"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert!(root.join(".git/hooks/pre-commit").exists());
        let (code, _, err) = run(&invocation(&["hooks", "--apply", "uninstall"]), &root);
        assert_eq!(code, 0, "{err}");
        assert!(!root.join(".git/hooks/pre-commit").exists());
    }

    #[test]
    fn unknown_verb_and_trigger_usage_name_every_verb_and_trigger() {
        let scratch = dx_test_scratch::scratch("hooks-usage-");
        let cases = [
            (
                vec!["hooks", "frobnicate"],
                format!("usage: dx hooks <{}>", dx_adopt::hook_verb_pipe()),
            ),
            (
                vec!["hooks", "run", "pre-rebase"],
                format!("usage: dx hooks run <{}>", dx_adopt::hook_trigger_pipe()),
            ),
        ];
        for (words, expected) in cases {
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &invocation(&words),
                    scratch.path(),
                    &NullQuery,
                    &NullRunner,
                    None,
                    &mut out,
                    &mut err
                ),
                2
            );
            assert!(out.is_empty());
            assert!(
                String::from_utf8(err)
                    .expect("err")
                    .contains(&format!("dx: {expected}")),
                "dx {} must print `{expected}`",
                words.join(" ")
            );
        }
    }

    #[test]
    fn hook_process_launch_failures_stop_before_recording_timings() {
        struct MissingProcess;
        impl QueryRunner for MissingProcess {
            fn run_query(&self, _: &[String], _: &Path) -> io::Result<QueryResult> {
                Err(io::Error::other("missing executable"))
            }
        }
        impl dx_process::Runner for MissingProcess {
            fn git_tool(&self) -> Option<PathBuf> {
                Some(PathBuf::from("/hermetic/git"))
            }
            fn run(
                &self,
                _: &[String],
                _: &Path,
                _: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                Err(io::Error::other("missing executable"))
            }
        }
        let scratch = dx_test_scratch::scratch("hooks-spawn-failure-");
        write_workspace(scratch.path());
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        for query in [
            &MissingProcess as &dyn QueryRunner,
            &ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n"),
        ] {
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &inv,
                    scratch.path(),
                    query,
                    &MissingProcess,
                    None,
                    &mut Vec::new(),
                    &mut err
                ),
                1
            );
            assert!(String::from_utf8(err)
                .expect("err")
                .contains("missing executable"));
            assert!(!scratch.path().join(dx_adopt::HOOK_TIMINGS_REL).exists());
        }
        assert_eq!(first_line(b"\nignored"), "ignored");
        assert_eq!(first_line(b""), "no Git diagnostic");
        assert_eq!(
            first_line("x".repeat(201).as_bytes()),
            format!("{}...", "x".repeat(200))
        );
    }

    #[test]
    fn a_wide_character_on_the_truncation_edge_is_not_split() {
        let mut stderr = "x".repeat(198);
        stderr.push('€');
        stderr.push_str("git: fatal: bad object HEAD");
        let got = first_line(stderr.as_bytes());
        assert_eq!(got, format!("{}...", "x".repeat(198)));
        assert!(got.len() <= 203, "bounded: {}", got.len());
        assert_eq!(first_line(b"  \nreal error"), "real error");
    }

    #[test]
    fn hooks_status_shows_merged_layers() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        assert!(text.contains("baseline:"));
        assert!(text.contains("overlay:"));
        assert!(text.contains("timings:"));
        assert!(text.contains("effective:"));
        assert!(!text.contains("p95"));
    }

    #[test]
    fn hooks_status_rejects_invalid_toml() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-bad-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.hooks.toml"), "not toml = [").expect("bad baseline");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("hooks status"));
    }

    #[test]
    fn hooks_status_shows_measured_timings() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-timed-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(
            root.join(".dx/hooks-timings.toml"),
            "[timings]\n\"format --check\" = 1.23\n",
        )
        .expect("timings");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        assert!(text.contains("1.23s (measured)"));
        assert!(!text.contains("p95 12s"));
    }

    #[test]
    fn hooks_dry_run_plans_without_mutating() {
        for (words, want) in [
            (vec!["hooks", "install", "--dry-run"], "would install"),
            (vec!["hooks", "uninstall", "--dry-run"], "would remove"),
            (vec!["hooks", "status", "--dry-run"], "would show"),
            (
                vec!["hooks", "run", "pre-commit", "--dry-run"],
                "would run pre-commit",
            ),
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-hooks-dry-");
            let root = scratch.path().to_path_buf();
            let (code, out, _err) = run(&inv, &root);
            assert_eq!(code, 0, "words: {words:?}");
            assert!(out.contains(want), "words: {words:?}");
            assert!(!root.join(".git/hooks/pre-commit").exists());
        }
        let inv = invocation(&["hooks", "status", "--dry-run", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-dry-quiet-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());
    }

    struct ScriptQuery {
        outputs: RefCell<Vec<crate::resolve::QueryResult>>,
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptQuery {
        fn staged_then_owners(staged: &str, owners: &str) -> Self {
            Self::scripted(&[staged], owners, "")
        }

        fn staged_then_attributed_owners(staged: &str, owners: &str, sources: &str) -> Self {
            Self::scripted(&[staged], owners, sources)
        }

        fn push_then_owners(diffs: &[&str], owners: &str) -> Self {
            Self::scripted(diffs, owners, "")
        }

        fn scripted(git_outputs: &[&str], owners: &str, sources: &str) -> Self {
            let mut outputs: Vec<crate::resolve::QueryResult> = git_outputs
                .iter()
                .map(|stdout| crate::resolve::QueryResult {
                    code: Some(0),
                    stdout: stdout.as_bytes().to_vec(),
                    stderr: Vec::new(),
                })
                .collect();
            outputs.push(crate::resolve::QueryResult {
                code: Some(0),
                stdout: owners.as_bytes().to_vec(),
                stderr: Vec::new(),
            });
            if !sources.is_empty() {
                outputs.push(crate::resolve::QueryResult {
                    code: Some(0),
                    stdout: sources.as_bytes().to_vec(),
                    stderr: Vec::new(),
                });
            }
            Self {
                outputs: RefCell::new(outputs),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl QueryRunner for ScriptQuery {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            self.seen.borrow_mut().push(argv.to_vec());
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    struct ScriptRunner {
        git: Option<PathBuf>,
        codes: RefCell<Vec<Option<i32>>>,
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptRunner {
        fn git_with_codes(git: &str, codes: Vec<Option<i32>>) -> Self {
            Self {
                git: Some(PathBuf::from(git)),
                codes: RefCell::new(codes),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl dx_process::Runner for ScriptRunner {
        fn git_tool(&self) -> Option<PathBuf> {
            self.git.clone()
        }

        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.seen.borrow_mut().push(argv.to_vec());
            Ok(dx_process::ChildStatus {
                code: self.codes.borrow_mut().remove(0),
            })
        }
    }

    fn write_workspace(root: &Path) {
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("build");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source");
    }

    #[test]
    fn hooks_run_handles_empty_selection_signal_and_invalid_state() {
        for (scenario, want_code, want_detail) in [
            ("no-checks", 0, "no checks configured"),
            ("deleted-file", 0, "staged 1 file(s) as 1 target(s)"),
            ("bad-config", 1, ""),
            ("signal", 1, "terminated by signal"),
            ("bad-timings", 1, ""),
            ("timings-collision", 1, "write timings"),
            ("query-failed", 1, "hook git diff failed"),
            ("query-utf8", 1, "UTF-8"),
            ("owner-failed", 1, "query"),
        ] {
            let scratch = dx_test_scratch::scratch("hooks-failure-");
            let root = scratch.path();
            write_workspace(root);
            let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
            let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
            match scenario {
                "no-checks" => {
                    std::fs::write(root.join("dx.hooks.toml"), "[hooks]\npre_commit = []\n")
                        .expect("config")
                }
                "deleted-file" => {
                    std::fs::remove_file(root.join("pkg/a.py")).expect("delete source")
                }
                "bad-config" => {
                    std::fs::write(root.join("dx.hooks.toml"), "[broken").expect("config")
                }
                "signal" => runner.codes.borrow_mut()[0] = None,
                "bad-timings" => {
                    std::fs::create_dir(root.join(".dx")).expect("dx");
                    std::fs::write(root.join(dx_adopt::HOOK_TIMINGS_REL), "[broken")
                        .expect("timings");
                }
                "timings-collision" => {
                    std::fs::create_dir_all(root.join(dx_adopt::HOOK_TIMINGS_REL))
                        .expect("collision")
                }
                "query-failed" => query.outputs.borrow_mut()[0].code = Some(1),
                "query-utf8" => query.outputs.borrow_mut()[0].stdout = vec![0xff],
                "owner-failed" => query.outputs.borrow_mut()[1].code = Some(1),
                _ => unreachable!(),
            }
            let mut out = Vec::new();
            let mut err = Vec::new();
            let words: &[&str] = match scenario {
                "bad-timings" | "timings-collision" => &["hooks", "--apply", "run", "pre-commit"],
                _ => &["hooks", "run", "pre-commit"],
            };
            let inv = invocation(words);
            assert_eq!(
                execute_hooks(&inv, root, &query, &runner, None, &mut out, &mut err),
                want_code,
                "{scenario}"
            );
            let detail = format!(
                "{}{}",
                String::from_utf8(out).expect("out"),
                String::from_utf8(err).expect("err")
            );
            assert!(detail.contains(want_detail), "{scenario}: {detail}");
            if want_code == 1 {
                assert!(!detail.is_empty());
            }
            if scenario == "signal" {
                assert_eq!(runner.seen.borrow().len(), 1);
            }
        }
    }

    #[test]
    fn hooks_status_rejects_invalid_overlay_and_timings() {
        for rel in [dx_adopt::HOOK_OVERLAY_REL, dx_adopt::HOOK_TIMINGS_REL] {
            let scratch = dx_test_scratch::scratch("hooks-status-invalid-");
            std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
            std::fs::write(scratch.path().join(rel), "[broken").expect("invalid layer");
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_status(
                    &invocation(&["hooks", "status"]),
                    scratch.path(),
                    &mut out,
                    &mut err
                ),
                1
            );
            assert!(out.is_empty());
            assert!(String::from_utf8(err)
                .expect("err")
                .contains("missing merged layer"));
        }
    }

    #[test]
    fn hooks_run_checks_without_writing_timings_by_default() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, out, _err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 0);
        assert_eq!(runner.seen.borrow().len(), 2);
        assert!(runner.seen.borrow()[0][1..].contains(&"format".to_owned()));
        let text = out;
        assert!(text.contains("format --check ok"));
        assert!(!text.contains("budget 120s)") || text.contains("/ budget 120s)"));
        assert!(!text.contains("ran pre-commit: ok (budget 120s)"));
        assert!(
            !root.join(".dx/hooks-timings.toml").exists(),
            "check-mode hooks must not write timings"
        );
        assert_eq!(query.seen.borrow().len(), 2);
        assert!(
            query.seen.borrow()[0][0].ends_with("git")
                || query.seen.borrow()[0][0] == "/hermetic/git"
        );
    }

    #[test]
    fn hooks_run_apply_records_measured_timings() {
        let inv = invocation(&["hooks", "--apply", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-apply-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, _, _err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 0);
        let timings =
            std::fs::read_to_string(root.join(".dx/hooks-timings.toml")).expect("timings");
        assert!(timings.contains("format --check"));
        assert!(!timings.contains("p95"));
    }

    #[test]
    fn hooks_run_config_explicit_apply_survives_and_records_timings() {
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-config-apply-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        std::fs::write(
            root.join("dx.hooks.toml"),
            "[hooks]\npre_commit = [\"format --apply\"]\n",
        )
        .expect("config");
        let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0)]);
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let (code, _, _err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 0);
        assert_eq!(runner.seen.borrow().len(), 1);
        assert!(
            runner.seen.borrow()[0].contains(&"--apply".to_owned()),
            "config-explicit --apply must reach the child: {:?}",
            runner.seen.borrow()[0]
        );
        assert!(
            root.join(".dx/hooks-timings.toml").exists(),
            "config-explicit apply records timings"
        );
    }

    #[test]
    fn hook_children_stay_read_only_without_explicit_apply() {
        let targets = vec!["//pkg:lib".to_owned()];
        for (check, want) in [
            ("format", vec!["/dx", "format", "//pkg:lib"]),
            ("update", vec!["/dx", "update", "--check", "//pkg:lib"]),
            ("generate", vec!["/dx", "generate", "--check", "//pkg:lib"]),
            (
                "format --check",
                vec!["/dx", "format", "--check", "//pkg:lib"],
            ),
            ("fix --apply", vec!["/dx", "fix", "--apply", "//pkg:lib"]),
            (
                "run //app:bin",
                vec!["/dx", "run", "//app:bin", "//pkg:lib"],
            ),
            ("docs --serve", vec!["/dx", "docs", "--serve", "//pkg:lib"]),
            (
                "bogus --check",
                vec!["/dx", "bogus", "--check", "//pkg:lib"],
            ),
        ] {
            let want: Vec<String> = want.into_iter().map(ToString::to_string).collect();
            assert_eq!(check_argv("/dx", check, &targets), want, "{check}");
        }
        assert!(!hook_applies(
            &invocation(&["hooks", "run", "pre-commit"]),
            &["format --check".to_owned()]
        ));
        assert!(hook_applies(
            &invocation(&["hooks", "--apply", "run", "pre-commit"]),
            &["format --check".to_owned()]
        ));
        assert!(hook_applies(
            &invocation(&["hooks", "run", "pre-commit"]),
            &["format --apply".to_owned()]
        ));
    }

    #[test]
    fn hooks_run_blocks_on_check_failure() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-fail-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(1), Some(0)]);
        let (code, _out, err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 1);
        assert!(err.contains("failed"));
    }

    #[test]
    fn hooks_run_blocks_on_budget_timeout() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-timeout-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        std::fs::write(root.join("dx.hooks.toml"), "[hooks]\nbudget_secs = 0\n")
            .expect("zero budget");
        let query = ScriptQuery::staged_then_owners("M\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, _out, err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 1);
        assert!(err.contains("exceeded budget"));
    }

    #[test]
    fn hooks_run_needs_hermetic_git() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-nogit-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let runner = ScriptRunner {
            git: None,
            codes: RefCell::new(vec![]),
            seen: RefCell::new(Vec::new()),
        };
        let (code, _out, err) = run_with_query(&inv, &root, &NullQuery);
        assert_eq!(code, 1);
        assert!(err.contains("hermetic"));
        assert!(runner.seen.borrow().is_empty());
    }

    #[test]
    fn hooks_run_passes_with_no_staged_files() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-empty-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery {
            outputs: RefCell::new(vec![crate::resolve::QueryResult {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            }]),
            seen: RefCell::new(Vec::new()),
        };
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![]);
        let (code, out, _err) = run_with(&inv, &root, &query, &runner);
        assert_eq!(code, 0);
        assert!(out.contains("no staged changes"));
        assert!(runner.seen.borrow().is_empty());
    }

    fn run_with_stdin(
        inv: &Invocation,
        root: &Path,
        query: &dyn QueryRunner,
        runner: &dyn dx_process::Runner,
        hook_stdin: Option<&[u8]>,
    ) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_hooks(inv, root, query, runner, hook_stdin, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    fn zero_sha() -> String {
        "0".repeat(40)
    }

    fn push_stdin(refs: &[(&str, &str, &str, &str)]) -> Vec<u8> {
        refs.iter()
            .map(|(local_ref, local, remote_ref, remote)| {
                format!("{local_ref} {local} {remote_ref} {remote}\n")
            })
            .collect::<String>()
            .into_bytes()
    }

    #[test]
    fn hooks_run_push_ignores_the_index_and_checks_pushed_paths() {
        let inv = invocation(&["hooks", "run", "pre-push"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-push-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let local = "a".repeat(40);
        let remote = "b".repeat(40);
        let stdin = push_stdin(&[("refs/heads/main", &local, "refs/heads/main", &remote)]);
        let query = ScriptQuery::push_then_owners(&["M\0pkg/a.py\0"], "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, out, err) = run_with_stdin(&inv, &root, &query, &runner, Some(&stdin));
        assert_eq!(code, 0, "{out}{err}");
        let git_argv = &query.seen.borrow()[0];
        assert!(
            git_argv.contains(&"--name-status".to_owned()),
            "{git_argv:?}"
        );
        assert!(git_argv.contains(&"-z".to_owned()), "{git_argv:?}");
        assert!(git_argv.contains(&remote), "{git_argv:?}");
        assert!(git_argv.contains(&local), "{git_argv:?}");
        assert!(!git_argv.contains(&"--cached".to_owned()), "{git_argv:?}");
        assert!(
            runner.seen.borrow()[0].contains(&"//pkg:lib".to_owned()),
            "{:?}",
            runner.seen.borrow()
        );
        assert!(out.contains("pushed 1 file(s) as 1 target(s)"), "{out}");
        assert!(out.contains("worktree files"), "{out}");
    }

    #[test]
    fn hooks_run_push_reports_empty_stdin_and_deletion_only_refs() {
        let inv = invocation(&["hooks", "run", "pre-push"]);
        for (name, stdin, want) in [
            ("empty-stdin", Vec::new(), "no pushed refs on stdin"),
            (
                "deletion-only",
                push_stdin(&[(
                    "refs/heads/gone",
                    &zero_sha(),
                    "refs/heads/gone",
                    &"c".repeat(40),
                )]),
                "no pushed changes",
            ),
        ] {
            let scratch = dx_test_scratch::scratch("dx-adopt-hooks-push-empty-");
            let root = scratch.path().to_path_buf();
            write_workspace(&root);
            let query = ScriptQuery::push_then_owners(&[], "//pkg:lib\n");
            let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![]);
            let (code, out, _err) = run_with_stdin(&inv, &root, &query, &runner, Some(&stdin));
            assert_eq!(code, 0, "{name}");
            assert!(out.contains(want), "{name}: {out}");
            assert!(runner.seen.borrow().is_empty(), "{name}");
        }
    }

    #[test]
    fn hooks_run_push_diffs_new_branches_against_the_empty_tree() {
        let inv = invocation(&["hooks", "run", "pre-push"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-push-new-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let local = "a".repeat(40);
        let stdin = push_stdin(&[
            ("refs/heads/new", &local, "refs/heads/new", &zero_sha()),
            (
                "refs/heads/gone",
                &zero_sha(),
                "refs/heads/gone",
                &"c".repeat(40),
            ),
        ]);
        let query = ScriptQuery::push_then_owners(&["A\0pkg/a.py\0"], "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, out, err) = run_with_stdin(&inv, &root, &query, &runner, Some(&stdin));
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(query.seen.borrow().len(), 2, "{:?}", query.seen.borrow());
        let git_argv = &query.seen.borrow()[0];
        assert!(
            git_argv.contains(&dx_adopt::EMPTY_TREE_SHA.to_owned()),
            "{git_argv:?}"
        );
        assert!(git_argv.contains(&local), "{git_argv:?}");
        assert!(
            runner.seen.borrow()[0].contains(&"//pkg:lib".to_owned()),
            "{:?}",
            runner.seen.borrow()
        );
        assert!(out.contains("pushed 1 file(s) as 1 target(s)"), "{out}");
    }

    #[test]
    fn hooks_run_push_rejects_malformed_stdin() {
        let inv = invocation(&["hooks", "run", "pre-push"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-push-bad-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::push_then_owners(&[], "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![]);
        let (code, _out, err) = run_with_stdin(
            &inv,
            &root,
            &query,
            &runner,
            Some(b"refs/heads/main only-two-fields\n"),
        );
        assert_eq!(code, 1);
        assert!(err.contains("four fields"), "{err}");
    }

    #[test]
    fn hooks_run_precommit_keeps_rename_identities_and_deleted_fallbacks() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-rename-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("R100\0pkg/was.py\0pkg/a.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, out, err) = run_with_stdin(&inv, &root, &query, &runner, None);
        assert_eq!(code, 0, "{out}{err}");
        let argv = &runner.seen.borrow()[0];
        assert!(argv.contains(&"//pkg:lib".to_owned()), "{argv:?}");
        assert!(argv.contains(&"//pkg/...".to_owned()), "{argv:?}");
        assert!(out.contains("staged 1 file(s) as 2 target(s)"), "{out}");
    }

    #[test]
    fn hooks_run_precommit_checks_spaced_and_build_inputs_verbatim() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-spaced-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        std::fs::write(root.join("pkg/space name.py"), "x = 1\n").expect("spaced");
        let query = ScriptQuery::staged_then_attributed_owners(
            "A\0pkg/space name.py\0M\0pkg/BUILD.bazel\0",
            "//pkg:lib\n",
            "//pkg:BUILD.bazel\n//pkg:space name.py\n",
        );
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let (code, out, err) = run_with_stdin(&inv, &root, &query, &runner, None);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("staged 2 file(s) as 1 target(s)"), "{out}");
        assert!(
            runner.seen.borrow()[0].contains(&"//pkg:lib".to_owned()),
            "{:?}",
            runner.seen.borrow()
        );
        let git_argv = &query.seen.borrow()[0];
        assert!(git_argv.contains(&"-z".to_owned()), "{git_argv:?}");
    }

    #[test]
    fn hooks_run_precommit_rejects_control_character_names_explicitly() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-control-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        std::fs::write(root.join("pkg/we\nird.py"), "x = 1\n").expect("newline name");
        let query = ScriptQuery::staged_then_owners("A\0pkg/we\nird.py\0", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![]);
        let (code, _out, err) = run_with_stdin(&inv, &root, &query, &runner, None);
        assert_eq!(code, 1);
        assert!(err.contains("control characters"), "{err}");
        assert!(runner.seen.borrow().is_empty());
    }

    struct RealGit {
        git: PathBuf,
        owners: String,
        owned_labels: RefCell<Vec<String>>,
        git_calls: RefCell<Vec<Vec<String>>>,
    }

    impl QueryRunner for RealGit {
        fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
            if argv
                .first()
                .is_some_and(|bin| bin == &self.git.to_string_lossy())
            {
                self.git_calls.borrow_mut().push(argv.to_vec());
                let output = std::process::Command::new(&self.git)
                    .args(&argv[1..])
                    .current_dir(cwd)
                    .output()
                    .expect("real git runs in fixtures");
                return Ok(QueryResult {
                    code: output.status.code(),
                    stdout: output.stdout,
                    stderr: output.stderr,
                });
            }
            if argv.last().is_some_and(|query| query.starts_with("deps(")) {
                let mut owned = self.owned_labels.borrow().clone();
                owned.sort();
                owned.dedup();
                return Ok(QueryResult {
                    code: Some(0),
                    stdout: owned.join("\n").as_bytes().to_vec(),
                    stderr: Vec::new(),
                });
            }
            if let Some(query) = argv.last() {
                let mut labels: Vec<String> = query
                    .split('"')
                    .skip(1)
                    .step_by(2)
                    .map(ToString::to_string)
                    .collect();
                labels.sort();
                labels.dedup();
                *self.owned_labels.borrow_mut() = labels;
            }
            Ok(QueryResult {
                code: Some(0),
                stdout: self.owners.as_bytes().to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    fn fixture_git() -> PathBuf {
        let system = PathBuf::from("/usr/bin/git");
        if system.exists() {
            system
        } else {
            PathBuf::from("git")
        }
    }

    fn git_fixture(git: &Path, root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new(git)
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("fixture git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout")
    }

    fn init_push_fixture(git: &Path, root: &Path) {
        git_fixture(git, root, &["init", "-q"]);
        git_fixture(git, root, &["config", "user.email", "hook@test"]);
        git_fixture(git, root, &["config", "user.name", "hook"]);
        git_fixture(git, root, &["config", "commit.gpgsign", "false"]);
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("build");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source");
        std::fs::write(root.join("pkg/old.py"), "x = 0\n").expect("deleted later");
        git_fixture(git, root, &["add", "-A"]);
        git_fixture(git, root, &["commit", "-qm", "base"]);
    }

    #[test]
    fn hooks_selection_uses_real_git_change_sets() {
        let git = fixture_git();
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-real-git-");
        let root = scratch.path().to_path_buf();
        init_push_fixture(&git, &root);
        std::fs::write(root.join("pkg/a.py"), "x = 2\n").expect("modify");
        std::fs::write(root.join("pkg/space name.py"), "y = 1\n").expect("spaced");
        std::fs::remove_file(root.join("pkg/old.py")).expect("delete");
        std::fs::write(root.join("pkg/BUILD.bazel"), "# touched\n").expect("build change");
        git_fixture(&git, &root, &["add", "-A"]);
        let runner = RealGit {
            git: git.clone(),
            owners: "//pkg:lib\n".to_owned(),
            owned_labels: RefCell::new(Vec::new()),
            git_calls: RefCell::new(Vec::new()),
        };
        let staged = staged_changes(&git, &root, &runner).expect("staged");
        let mut paths: Vec<String> = staged.iter().map(|change| change.path.clone()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "pkg/BUILD.bazel".to_owned(),
                "pkg/a.py".to_owned(),
                "pkg/old.py".to_owned(),
                "pkg/space name.py".to_owned(),
            ],
            "{staged:?}"
        );
        let deleted = staged
            .iter()
            .find(|change| change.path == "pkg/old.py")
            .expect("deleted change");
        assert_eq!(deleted.kind, dx_adopt::ChangeKind::Deleted);
        let targets = change_targets(&staged, &root, &runner, &[]).expect("targets");
        assert!(targets.contains(&"//pkg:lib".to_owned()), "{targets:?}");
        assert!(targets.contains(&"//pkg/...".to_owned()), "{targets:?}");

        git_fixture(&git, &root, &["commit", "-qm", "staged work"]);
        std::fs::write(root.join("pkg/b.py"), "z = 3\n").expect("pushed file");
        git_fixture(&git, &root, &["add", "-A"]);
        git_fixture(&git, &root, &["commit", "-qm", "pushed work"]);
        let remote = git_fixture(&git, &root, &["rev-parse", "HEAD~1"]);
        let local = git_fixture(&git, &root, &["rev-parse", "HEAD"]);
        let stdin = push_stdin(&[(
            "refs/heads/main",
            local.trim(),
            "refs/heads/main",
            remote.trim(),
        )]);
        let refs = dx_adopt::parse_push_refs(&stdin).expect("refs");
        let push_calls = runner.git_calls.borrow().len();
        let pushed = pushed_changes(&git, &root, &runner, &refs).expect("pushed");
        let paths: Vec<String> = pushed.iter().map(|change| change.path.clone()).collect();
        assert_eq!(paths, vec!["pkg/b.py".to_owned()], "{pushed:?}");
        assert!(
            runner.git_calls.borrow()[push_calls..]
                .iter()
                .all(|argv| !argv.contains(&"--cached".to_owned())),
            "{:?}",
            runner.git_calls.borrow()
        );
    }
}
