use super::super::common::{
    check_stdout_write, emit_built, frozen_summary, offline_summary, pre_exec, Env,
};
use super::{
    record_backend_error, record_named, runner_status, unavailable_message, RunMode, SetDetail,
    SlotGate,
};
use crate::args::Invocation;
use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, ResolvedTarget, Selection};
use std::collections::BTreeMap;

pub(super) fn execute_configured(
    invocation: &Invocation,
    env: Env<'_>,
    registry: &dependency_sets::Registry,
    verbose: bool,
    mode: RunMode,
) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let resolved = match dependency_sets::resolve(registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let summary = frozen_summary(
        offline_summary(display_summary(&resolved, mode), invocation.offline),
        invocation.frozen,
    );
    if invocation.output == dx_output::OutputMode::Json {
        if let Err(exit) = emit_built(
            out,
            dx_output::command_started(invocation.command.name(), mode.mode_text()),
        ) {
            return exit;
        }
    } else if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<String, SetDetail> = BTreeMap::new();
    let mut gate = SlotGate {
        command: invocation.command,
        err: &mut *err,
        events: &mut *out,
        asked: false,
        slot: None,
    };
    for target in &resolved {
        if let Err(signo) = run_configured_target(
            &mut attempted,
            &mut details,
            target,
            runner,
            workspace,
            invocation,
            &mut gate,
        ) {
            drop(gate);
            let _ = writeln!(err, "dx: cancelled: stopped by signal {signo}");
            return dx_process::interrupted_code(signo);
        }
    }
    drop(gate.slot.take());
    drop(gate);
    let selected: Vec<String> = resolved
        .iter()
        .map(|target| target.set.name.clone())
        .collect();
    let locks: BTreeMap<String, Vec<String>> = resolved
        .iter()
        .map(|target| (target.set.name.clone(), target.set.locks.clone()))
        .collect();
    super::finish_update(super::FinishUpdate {
        invocation,
        out,
        err,
        selected,
        attempted,
        details,
        verbose,
        mode,
        locks,
    })
}

fn run_configured_target(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<String, SetDetail>,
    target: &ResolvedTarget,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    invocation: &Invocation,
    gate: &mut SlotGate<'_>,
) -> Result<(), i32> {
    let mode = RunMode::of(invocation);
    let set = &target.set;
    let verb = mode.verb();
    if mode == RunMode::Update {
        if let Err(error) = dependency_sets::check_writable(set) {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Unsupported,
                format!("cannot update {}: {error}", set.name),
            );
            return Ok(());
        }
        if let Err(error) = dependency_sets::check_files(workspace, set, false) {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Failed,
                format!("failed to {verb} {}: {error}", set.name),
            );
            return Ok(());
        }
    } else if let Err(error) = dependency_sets::check_files(workspace, set, true) {
        record_named(
            attempted,
            details,
            &set.name,
            dx_update::outcome::SetStatus::Failed,
            format!("failed to {verb} {}: {error}", set.name),
        );
        return Ok(());
    }
    if let Err(error) = dependency_sets::check_backend_settings(workspace, set) {
        let status = match error {
            dependency_sets::DependencySetsError::UnsupportedSettings { .. }
            | dependency_sets::DependencySetsError::UnsupportedNpmLayout { .. }
            | dependency_sets::DependencySetsError::MissingPinnedLock { .. } => {
                dx_update::outcome::SetStatus::Unsupported
            }
            _ => dx_update::outcome::SetStatus::Failed,
        };
        record_named(
            attempted,
            details,
            &set.name,
            status,
            format!("cannot {verb} {}: {error}", set.name),
        );
        return Ok(());
    }
    let closure = match set.ecosystem {
        Ecosystem::Uv => dx_update::backend::UvClosure {
            uv: pinned(runner, dx_update::backend::UV_BIN_VAR).filter(|path| {
                runner
                    .tool_version(std::path::Path::new(path))
                    .is_some_and(|line| {
                        let name_and_version: Vec<&str> = line.split_whitespace().take(2).collect();
                        name_and_version.join(" ") == dx_update::backend::UV_VERSION_LINE
                    })
            }),
            python: pinned(runner, dx_update::backend::UV_PYTHON_VAR),
        },
        Ecosystem::Npm | Ecosystem::Bazel | Ecosystem::Ruby | Ecosystem::PowerShell => {
            dx_update::backend::UvClosure::default()
        }
    };
    let head = invocation.bazel_head();
    let planned = match mode {
        RunMode::Update => dx_update::backend::plan_configured(
            workspace,
            set,
            &target.request,
            invocation.offline,
            invocation.frozen,
            &head,
            &closure,
        )
        .map(|plan| match plan {
            dx_update::backend::BackendPlan::Run { argv, env } => {
                dx_update::backend::CheckPlan::Run { argv, env }
            }
            dx_update::backend::BackendPlan::Noop => dx_update::backend::CheckPlan::Pinned,
        }),
        RunMode::Check => dx_update::backend::check_configured(
            workspace,
            set,
            &target.request,
            invocation.offline,
            invocation.frozen,
            &head,
            &closure,
        ),
    };
    let (argv, extra) = match planned {
        Err(error) => {
            record_backend_error(attempted, details, &error, verb, &set.name);
            return Ok(());
        }
        Ok(dx_update::backend::CheckPlan::Pinned) => {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Pinned,
                pinned_line(set),
            );
            return Ok(());
        }
        Ok(dx_update::backend::CheckPlan::Unavailable) => {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Unsupported,
                unavailable_message(&set.name),
            );
            return Ok(());
        }
        Ok(dx_update::backend::CheckPlan::Run { argv, env }) => (argv, env),
    };
    let mut env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let bazel = dx_update::backend::runs_bazel(&argv);
    let windows_env: Vec<(&str, String)> = if cfg!(windows) && !bazel {
        ["SYSTEMROOT", "TEMP", "TMP", "USERPROFILE"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
            .collect()
    } else {
        Vec::new()
    };
    for (key, value) in &windows_env {
        env_refs.push((key, value.as_str()));
    }
    gate.before(&argv)?;
    let launched = if bazel {
        match gate.slot.as_ref() {
            Some(slot) => {
                dx_process::Runner::run(&slot.runner(runner), &argv, workspace, &env_refs)
            }
            None => runner.run(&argv, workspace, &env_refs),
        }
    } else {
        runner.run_hermetic(&argv, workspace, &env_refs)
    };
    let ok = match mode {
        RunMode::Update => (
            dx_update::outcome::SetStatus::Updated,
            success_line(set, &target.request),
        ),
        RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
    };
    let (status, message) = runner_status(launched, verb, &set.name, ok);
    record_named(attempted, details, &set.name, status, message);
    Ok(())
}

fn pinned(runner: &dyn dx_process::Runner, var: &str) -> Option<String> {
    runner
        .pinned_tool(var)
        .map(|path| path.to_string_lossy().into_owned())
}

fn display_summary(resolved: &[ResolvedTarget], mode: RunMode) -> String {
    let command = match mode {
        RunMode::Update => "Running update for",
        RunMode::Check => "Running update --check for",
    };
    let mut parts = Vec::new();
    for target in resolved {
        match &target.request {
            Selection::Full => parts.push(target.set.name.clone()),
            Selection::Packages(packages) => {
                for package in packages {
                    parts.push(format!("{}:{package}", target.set.name));
                }
            }
        }
    }
    format!("{command} {}", parts.join(", "))
}

fn current_line(set: &ResolvedSet) -> String {
    format!("{} lockfile current ({})", set.name, set.locks.join(", "))
}

fn pinned_line(set: &ResolvedSet) -> String {
    format!("{} pins are manual; nothing to resolve", set.name)
}

fn success_line(set: &ResolvedSet, request: &Selection) -> String {
    match request {
        Selection::Full => format!("updated {} ({})", set.name, set.locks.join(", ")),
        Selection::Packages(packages) => {
            let selections: Vec<String> = packages
                .iter()
                .map(|package| format!("{}:{package}", set.name))
                .collect();
            format!(
                "updated {} ({})",
                selections.join(", "),
                set.locks.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::Harness;
    use super::super::super::test_support::{event, event_kinds, json_events};
    use super::super::update_live::{fake_pinned, Launch, ScriptRunner};

    fn uv_call(check: bool, dir: &str) -> Vec<String> {
        let mut argv = vec![fake_pinned("uv"), "lock".to_owned()];
        if check {
            argv.push("--check".to_owned());
        }
        for word in [
            "--directory",
            dir,
            "--no-config",
            "--no-cache",
            "--no-python-downloads",
            "--python",
        ] {
            argv.push(word.to_owned());
        }
        argv.push(fake_pinned("python3"));
        argv
    }

    const DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "worker"
ecosystem = "uv"
manifests = ["services/worker/pyproject.toml"]
locks = ["services/worker/uv.lock"]
scopes = ["services/worker"]
"#;

    const PYPROJECT: &str = r#"
[project]
name = "project"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
"#;

    const UV_LOCK: &str = r#"
version = 1
requires-python = ">=3.9"

[[package]]
name = "anyio"
version = "4.0.0"
source = { registry = "https://pypi.org/simple" }
"#;

    fn consumer_harness(name: &str, dx_toml: &str, with_locks: bool) -> Harness {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let harness = Harness::new(&format!("update-consumer-{name}-{id}"));
        harness.write_source("dx.toml", dx_toml);
        harness.write_source("apps/frontend/pyproject.toml", PYPROJECT);
        harness.write_source("services/worker/pyproject.toml", PYPROJECT);
        if with_locks {
            harness.write_source("apps/frontend/uv.lock", UV_LOCK);
            harness.write_source("services/worker/uv.lock", UV_LOCK);
        }
        harness
    }

    fn run_with(argv: &[&str], runner: &ScriptRunner, harness: &Harness) -> (i32, String, String) {
        let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        let invocation = crate::args::parse(&words).expect("parse");
        harness.execute_with(&invocation, runner)
    }

    fn run_consumer(argv: &[&str], runner: &ScriptRunner) -> (i32, String, String, Harness) {
        let harness = consumer_harness("sets", DX_TOML, true);
        let (code, out, err) = run_with(argv, runner, &harness);
        (code, out, err, harness)
    }

    #[test]
    fn consumer_bare_check_runs_configured_sets_and_no_phantoms() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--check"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Running update --check for frontend, worker"),
            "{out}"
        );
        assert!(
            out.contains("frontend lockfile current (apps/frontend/uv.lock)"),
            "{out}"
        );
        assert!(
            out.contains("worker lockfile current (services/worker/uv.lock)"),
            "{out}"
        );
        for phantom in [
            "cargo",
            "pnpm",
            "maven",
            "nuget",
            "examples/",
            "third_party/",
            "quality/tools",
        ] {
            assert!(!out.contains(phantom), "phantom {phantom} in {out}");
        }
        assert_eq!(err, "", "{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![
                uv_call(true, "apps/frontend"),
                uv_call(true, "services/worker")
            ]
        );
        assert_eq!(runner.hermetic_envs.borrow().len(), 2);
        for env in runner.hermetic_envs.borrow().iter() {
            assert!(
                env.iter().all(|(key, _)| cfg!(windows)
                    && ["SYSTEMROOT", "TEMP", "TMP", "USERPROFILE"].contains(&key.as_str())),
                "{env:?}"
            );
        }
    }

    #[test]
    fn consumer_without_pinned_uv_is_unavailable_and_never_clean() {
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let (code, out, err, _) = run_consumer(&["update", "--check"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(err.contains("DX_UV_BIN"), "{err}");
        assert!(err.contains("PATH is never searched"), "{err}");
        assert!(!out.contains("lockfile current"), "{out}");
        assert!(runner.calls.borrow().is_empty());
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let (code, out, err, _) = run_consumer(&["update", "--apply", "worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(!out.contains("updated worker"), "{out}");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn consumer_backend_unavailable_reports_its_code() {
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let (code, out, err, _) = run_consumer(&["update", "--check", "--output=json"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"backend_unavailable\""), "{out}");
        assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
        for args in [
            &["update", "--check"][..],
            &["update", "--apply", "worker"][..],
        ] {
            let mut runner = ScriptRunner::new(&[]);
            runner.pinned = false;
            let (code, out, err, _) = run_consumer(args, &runner);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains("dx: backend_unavailable:"), "{err}");
            assert!(!err.contains("dx: update_failed:"), "{err}");
        }
    }

    #[test]
    fn consumer_uv_of_another_version_is_unavailable_and_never_clean() {
        for version in [None, Some("uv 0.11.0".to_owned())] {
            let mut runner = ScriptRunner::new(&[]);
            runner.version = version;
            let (code, out, err, _) = run_consumer(&["update", "--check"], &runner);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains("backend_unavailable"), "{err}");
            assert!(err.contains("uv 0.12.23"), "{err}");
            assert!(!out.contains("lockfile current"), "{out}");
            assert!(runner.calls.borrow().is_empty());
        }
    }

    #[test]
    fn consumer_failed_uv_fails_its_set_and_keeps_the_others() {
        let runner = ScriptRunner::new(&[("consumer-uv:apps/frontend", Some(1))]);
        let (code, out, err, _) = run_consumer(&["update", "--check"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to check frontend: updater exited 1"),
            "{err}"
        );
        assert!(
            out.contains("worker lockfile current (services/worker/uv.lock)"),
            "{out}"
        );
        assert!(!out.contains("frontend lockfile current"), "{out}");
        assert_eq!(runner.calls.borrow().len(), 2);
        let runner = ScriptRunner::new(&[("consumer-uv:services/worker", None)]);
        let (code, out, err, _) = run_consumer(&["update", "--apply", "worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to update worker: updater terminated by signal"),
            "{err}"
        );
        let mut runner = ScriptRunner::new(&[]);
        runner.io_error = true;
        let (code, out, err, _) = run_consumer(&["update", "--check", "worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to launch updater"), "{err}");
    }

    #[test]
    fn consumer_uv_settings_outside_the_closure_are_unsupported() {
        let harness = consumer_harness("settings", DX_TOML, true);
        harness.write_source(
            "apps/frontend/uv.toml",
            "exclude-newer = \"2020-01-01T00:00:00Z\"\n",
        );
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("apps/frontend/uv.toml exists"), "{err}");
        assert!(
            out.contains("worker lockfile current (services/worker/uv.lock)"),
            "{out}"
        );
        assert_eq!(
            *runner.calls.borrow(),
            vec![uv_call(true, "services/worker")]
        );
        let harness = consumer_harness("tool-uv", DX_TOML, true);
        harness.write_source(
            "services/worker/pyproject.toml",
            &format!("{PYPROJECT}\n[tool.uv]\nindex-url = \"https://example.invalid/simple\"\n"),
        );
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "worker"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("[tool.uv] index-url"), "{err}");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn consumer_partial_update_restores_its_own_locks() {
        let runner = ScriptRunner::new(&[("consumer-uv:services/worker", Some(1))]);
        let (code, out, err, _) = run_consumer(&["update", "--apply"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_recovery"), "{err}");
        assert!(err.contains("dx update --apply worker"), "{err}");
        assert!(
            err.contains("git checkout -- apps/frontend/uv.lock"),
            "{err}"
        );
        assert!(!err.contains("nothing to roll back"), "{err}");
    }

    #[test]
    fn consumer_set_named_like_builtin_restores_dx_toml_locks() {
        let toml = DX_TOML
            .replace("name = \"frontend\"", "name = \"uv\"")
            .replace("name = \"worker\"", "name = \"cargo\"");
        let harness = consumer_harness("builtin-names", &toml, true);
        let runner = ScriptRunner::new(&[("consumer-uv:services/worker", Some(1))]);
        let (code, out, err) = run_with(&["update", "--apply"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("git checkout -- apps/frontend/uv.lock"),
            "{err}"
        );
        assert!(!err.contains("python/tests/fixtures"), "{err}");
        assert!(!err.contains("Cargo.lock"), "{err}");
    }

    #[test]
    fn consumer_selective_update_runs_one_directory() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--apply", "worker"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running update for worker"), "{out}");
        assert!(
            out.contains("updated worker (services/worker/uv.lock)"),
            "{out}"
        );
        assert!(!out.contains("frontend"), "{out}");
        assert_eq!(err, "", "{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![uv_call(false, "services/worker")]
        );
    }

    #[test]
    fn consumer_scopes_resolve_to_their_owning_sets() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) =
            run_consumer(&["update", "--check", "apps/frontend/app/main.py"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(runner.calls.borrow()[0].contains(&"apps/frontend".to_owned()));
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--check", "//..."], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 2);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) =
            run_consumer(&["update", "--check", "//services/worker/..."], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(runner.calls.borrow()[0].contains(&"services/worker".to_owned()));
    }

    #[test]
    fn consumer_unknown_and_unowned_selectors_fail_closed() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) = run_consumer(&["update", "--apply", "cargo"], &runner);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("unknown dependency set or scope"), "{err}");
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) = run_consumer(&["update", "--apply", "docs/cli/README.md"], &runner);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("no owning dependency set"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_selective_package_is_unsupported_without_launch() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) = run_consumer(&["update", "--apply", "frontend:anyio"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("frontend"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_missing_lock_fails_check_without_launch() {
        let harness = consumer_harness("missing-lock", DX_TOML, false);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--check", "worker"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
        assert!(err.contains("is missing"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_read_only_set_refuses_update_but_checks() {
        const READONLY: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
writable = false
"#;
        let harness = consumer_harness("read-only", READONLY, true);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--apply", "frontend"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("read-only"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check", "frontend"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 1);
    }

    #[test]
    fn consumer_check_leaves_the_workspace_untouched() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, _, harness) = run_consumer(&["update", "--check"], &runner);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read(harness.workspace.join("apps/frontend/uv.lock")).expect("lock"),
            UV_LOCK.as_bytes()
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml"))
                .expect("manifest"),
            PYPROJECT.as_bytes()
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("dx.toml")).expect("config"),
            DX_TOML.as_bytes()
        );
    }

    #[test]
    fn consumer_offline_update_needs_network() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) =
            run_consumer(&["update", "--apply", "--offline", "worker"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_frozen_update_locks_resolution() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) = run_consumer(&["update", "--apply", "--frozen", "worker"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        assert!(!err.contains("offline_required"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) =
            run_consumer(&["update", "--check", "--frozen", "worker"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("worker lockfile current (services/worker/uv.lock)"),
            "{out}{err}"
        );
    }

    #[test]
    fn consumer_invalid_config_fails_before_any_backend() {
        let harness = consumer_harness("invalid", "schema_version = 1\n", true);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("dx.toml"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_json_check_reports_per_set_correlations() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--check", "--output=json"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(out.contains("\"correlation\":\"update:frontend\""), "{out}");
        assert!(out.contains("\"correlation\":\"update:worker\""), "{out}");
        let _ = event(&events, "command_started");
    }

    const MIXED_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "jobs"
ecosystem = "npm"
manifests = ["consumer-npm/jobs/package.json"]
locks = ["consumer-npm/jobs/pnpm-lock.yaml"]
scopes = ["consumer-npm/jobs"]

[[dependency_set]]
name = "web"
ecosystem = "npm"
manifests = ["consumer-npm/web/package.json"]
locks = ["consumer-npm/web/pnpm-lock.yaml"]
scopes = ["consumer-npm/web"]
"#;

    const PACKAGE_JSON: &str = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\"\n}\n";

    const PNPM_LOCK: &str = "lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n";

    fn mixed_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, MIXED_TOML, true);
        for dir in ["consumer-npm/jobs", "consumer-npm/web"] {
            harness.write_source(&format!("{dir}/package.json"), PACKAGE_JSON);
            harness.write_source(&format!("{dir}/pnpm-lock.yaml"), PNPM_LOCK);
        }
        harness
    }

    fn npm_env() -> Vec<(String, String)> {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        vec![
            ("npm_config_globalconfig".to_owned(), null.to_owned()),
            ("npm_config_userconfig".to_owned(), null.to_owned()),
        ]
    }

    fn npm_call(
        harness: &Harness,
        startup: &[String],
        offline: bool,
        dir: &str,
        command: &[&str],
    ) -> Vec<String> {
        let mut argv = dx_process::startup_argv(startup);
        argv.push("run".to_owned());
        argv.extend(dx_process::build_verb_options(
            "run",
            &["--config=dx_dev".to_owned()],
        ));
        argv.push("--config=dx_dev".to_owned());
        if offline {
            argv.push("--repository_disable_download".to_owned());
        }
        for word in ["@pnpm//:pnpm", "--", "--dir"] {
            argv.push(word.to_owned());
        }
        argv.push(harness.workspace.join(dir).to_string_lossy().into_owned());
        argv.extend(command.iter().map(|word| (*word).to_owned()));
        argv
    }

    fn npm_check(harness: &Harness, dir: &str) -> Vec<String> {
        npm_call(
            harness,
            &[],
            false,
            dir,
            &["install", "--lockfile-only", "--frozen-lockfile"],
        )
    }

    fn read_bytes(harness: &Harness, rel: &str) -> Vec<u8> {
        std::fs::read(harness.workspace.join(rel)).unwrap_or_else(|error| panic!("{rel}: {error}"))
    }

    #[test]
    fn consumer_mixed_check_runs_uv_hermetic_and_npm_through_bazel() {
        let harness = mixed_harness("mixed-check");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("jobs lockfile current (consumer-npm/jobs/pnpm-lock.yaml)"),
            "{out}"
        );
        assert!(
            out.contains("web lockfile current (consumer-npm/web/pnpm-lock.yaml)"),
            "{out}"
        );
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 3);
        assert_eq!(launches[0].argv, uv_call(true, "apps/frontend"));
        assert!(launches[0].hermetic);
        assert_eq!(
            launches[1..],
            [
                Launch {
                    argv: npm_check(&harness, "consumer-npm/jobs"),
                    cwd: harness.workspace.clone(),
                    env: npm_env(),
                    hermetic: false,
                },
                Launch {
                    argv: npm_check(&harness, "consumer-npm/web"),
                    cwd: harness.workspace.clone(),
                    env: npm_env(),
                    hermetic: false,
                },
            ]
        );
        drop(launches);
        for rel in [
            "consumer-npm/jobs/pnpm-lock.yaml",
            "consumer-npm/web/pnpm-lock.yaml",
        ] {
            assert_eq!(read_bytes(&harness, rel), PNPM_LOCK.as_bytes(), "{rel}");
        }
        assert!(!harness
            .workspace
            .join("consumer-npm/web/node_modules")
            .exists());
    }

    #[test]
    fn consumer_npm_apply_runs_one_set_and_leaves_the_others() {
        let harness = mixed_harness("npm-apply");
        let before: Vec<Vec<u8>> = [
            "consumer-npm/jobs/package.json",
            "consumer-npm/jobs/pnpm-lock.yaml",
            "apps/frontend/uv.lock",
        ]
        .iter()
        .map(|rel| read_bytes(&harness, rel))
        .collect();
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "web"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("updated web (consumer-npm/web/pnpm-lock.yaml)"),
            "{out}"
        );
        assert_eq!(
            *runner.launches.borrow(),
            vec![Launch {
                argv: npm_call(
                    &harness,
                    &[],
                    false,
                    "consumer-npm/web",
                    &["install", "--lockfile-only"]
                ),
                cwd: harness.workspace.clone(),
                env: npm_env(),
                hermetic: false,
            }]
        );
        let after: Vec<Vec<u8>> = [
            "consumer-npm/jobs/package.json",
            "consumer-npm/jobs/pnpm-lock.yaml",
            "apps/frontend/uv.lock",
        ]
        .iter()
        .map(|rel| read_bytes(&harness, rel))
        .collect();
        assert_eq!(before, after);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "web:@types/node", "web:react"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("updated web:@types/node, web:react (consumer-npm/web/pnpm-lock.yaml)"),
            "{out}"
        );
        assert_eq!(
            *runner.calls.borrow(),
            vec![npm_call(
                &harness,
                &[],
                false,
                "consumer-npm/web",
                &["update", "@types/node", "react", "--lockfile-only"]
            )]
        );
    }

    #[test]
    fn consumer_npm_startup_options_and_offline_check_reach_the_bazel_run() {
        let harness = mixed_harness("npm-startup");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &[
                "update",
                "--check",
                "--offline",
                "--bazel-startup-option=--output_base=/tmp/npm-ob",
                "web",
            ],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![npm_call(
                &harness,
                &["--output_base=/tmp/npm-ob".to_owned()],
                true,
                "consumer-npm/web",
                &[
                    "install",
                    "--lockfile-only",
                    "--frozen-lockfile",
                    "--offline"
                ]
            )]
        );
    }

    #[test]
    fn consumer_npm_bazel_run_sees_the_held_slot() {
        let harness = mixed_harness("npm-slot");
        let scratch = dx_test_scratch::scratch("dx-update-consumer-slot-");
        let gate = crate::slots::Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), false);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = crate::slots::with_gate(Some(gate), || {
            run_with(&["update", "--check", "web"], &runner, &harness)
        });
        assert_eq!(code, 0, "{out}{err}");
        let marked = (crate::slots::SLOT_ENV_VAR.to_owned(), "1".to_owned());
        let launches = runner.launches.borrow();
        assert!(!launches.is_empty());
        for launch in launches.iter() {
            assert!(launch.env.contains(&marked), "{launch:?}");
        }
    }

    #[test]
    fn consumer_npm_failures_stay_with_their_own_set() {
        let harness = mixed_harness("npm-fail");
        let runner = ScriptRunner::new(&[("consumer-npm:web", Some(1))]);
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to check web: updater exited 1"),
            "{err}"
        );
        assert!(!out.contains("web lockfile current"), "{out}");
        assert!(
            out.contains("jobs lockfile current (consumer-npm/jobs/pnpm-lock.yaml)"),
            "{out}"
        );
        assert!(
            out.contains("frontend lockfile current (apps/frontend/uv.lock)"),
            "{out}"
        );
        assert_eq!(runner.calls.borrow().len(), 3);
    }

    #[test]
    fn consumer_npm_refusals_never_launch_pnpm() {
        let harness = mixed_harness("npm-refuse");
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--apply", "--frozen", "web"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--offline", "web"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = run_with(&["update", "--check", "web:react"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        let (code, _, err) = run_with(&["update", "--apply", "web:React"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(
            err.contains("invalid package \"React\" for set web"),
            "{err}"
        );
        assert!(runner.calls.borrow().is_empty());
        std::fs::remove_file(harness.workspace.join("consumer-npm/web/pnpm-lock.yaml"))
            .expect("drop lock");
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("consumer-npm/web/pnpm-lock.yaml"), "{err}");
        assert!(err.contains("is missing"), "{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![
                uv_call(true, "apps/frontend"),
                npm_check(&harness, "consumer-npm/jobs")
            ]
        );
    }

    #[test]
    fn consumer_npm_parent_pnpm_workspace_is_unsupported() {
        let harness = mixed_harness("npm-parent");
        harness.write_source("consumer-npm/pnpm-workspace.yaml", "packages: [\"*\"]\n");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "web"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(
            err.contains(
                "consumer-npm/pnpm-workspace.yaml makes pnpm write the parent workspace lock"
            ),
            "{err}"
        );
        assert!(runner.calls.borrow().is_empty());
    }

    const PINNED_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]

[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
"#;

    const PINNED_FILES: [(&str, &str); 4] = [
        (
            "apps/rb/Gemfile",
            "source \"https://rubygems.org\"\ngem \"rspec-core\"\n",
        ),
        (
            "apps/rb/Gemfile.lock",
            "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec-core (3.13.0)\n",
        ),
        (
            "tools/ps/PSGallery.requirements.psd1",
            "@{ Pester = '5.5.0' }\n",
        ),
        ("tools/ps/PSGallery.lock.json", "{}\n"),
    ];

    fn pinned_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, PINNED_TOML, true);
        for (rel, text) in PINNED_FILES {
            harness.write_source(rel, text);
        }
        harness
    }

    fn assert_pinned_files_unchanged(harness: &Harness) {
        for (rel, text) in PINNED_FILES {
            assert_eq!(read_bytes(harness, rel), text.as_bytes(), "{rel}");
        }
    }

    #[test]
    fn consumer_ruby_and_powershell_sets_report_pinned_and_launch_nothing() {
        for (argv, uv) in [
            (vec!["update", "--check"], uv_call(true, "apps/frontend")),
            (
                vec!["update", "--check", "--offline", "--frozen"],
                uv_call(true, "apps/frontend"),
            ),
            (vec!["update", "--apply"], uv_call(false, "apps/frontend")),
        ] {
            let harness = pinned_harness("pinned");
            let runner = ScriptRunner::new(&[]);
            let mut json = argv.clone();
            json.push("--output=json");
            let (code, out, err) = run_with(&json, &runner, &harness);
            let launches: Vec<Vec<String>> = runner
                .launches
                .borrow()
                .iter()
                .map(|launch| launch.argv.clone())
                .collect();
            assert!(
                launches.iter().all(|argv| argv[0] == fake_pinned("uv")),
                "only uv runs: {launches:?}"
            );
            if !argv.contains(&"--offline") {
                assert_eq!(code, 0, "{argv:?} {out}{err}");
                assert_eq!(launches, vec![uv.clone()], "{argv:?}");
            }
            let events = json_events(&out);
            for set in ["gems", "modules"] {
                assert!(
                    events
                        .iter()
                        .any(|event| event["code"] == "update_set_pinned"
                            && event["correlation"] == format!("update:{set}")),
                    "{argv:?} {set}: {out}"
                );
            }
            assert!(!out.contains("gems lockfile current"), "{out}");
            assert!(!out.contains("updated gems"), "{out}");
            assert_pinned_files_unchanged(&harness);
        }
        let harness = pinned_harness("pinned-text");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("gems pins are manual; nothing to resolve"),
            "{out}"
        );
        assert!(
            out.contains("modules pins are manual; nothing to resolve"),
            "{out}"
        );
    }

    #[test]
    fn consumer_pinned_package_selection_is_unsupported_without_fallback() {
        for (selector, regenerate) in [
            ("gems:rspec-core", "bundle lock"),
            ("modules:Pester", "PSGallery.lock.json by hand"),
        ] {
            for mode in ["--apply", "--check"] {
                let harness = pinned_harness("pinned-selective");
                let runner = ScriptRunner::new(&[]);
                let (code, out, err) = run_with(&["update", mode, selector], &runner, &harness);
                assert_ne!(code, 0, "{selector} {mode}: {out}{err}");
                assert!(err.contains("update_set_unsupported"), "{err}");
                assert!(err.contains(regenerate), "{err}");
                assert!(!out.contains("pins are manual"), "{out}");
                assert!(runner.launches.borrow().is_empty());
                assert_pinned_files_unchanged(&harness);
            }
        }
    }

    #[test]
    fn consumer_pinned_set_without_its_lock_declared_is_unsupported() {
        let harness = consumer_harness(
            "pinned-lock",
            "schema_version = 1\n[[dependency_set]]\nname = \"gems\"\necosystem = \"ruby\"\nmanifests = [\"apps/rb/Gemfile\"]\nlocks = [\"apps/rb/gems.locked\"]\nscopes = [\"apps/rb\"]\n",
            true,
        );
        harness.write_source("apps/rb/Gemfile", PINNED_FILES[0].1);
        harness.write_source("apps/rb/gems.locked", PINNED_FILES[1].1);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("apps/rb/Gemfile.lock"), "{err}");
        assert!(!out.contains("pins are manual"), "{out}");
        assert!(runner.launches.borrow().is_empty());
    }

    #[test]
    fn consumer_npm_wrong_manifest_name_exits_two() {
        let harness = consumer_harness(
            "npm-manifest",
            "schema_version = 1\n[[dependency_set]]\nname = \"web\"\necosystem = \"npm\"\nmanifests = [\"web/pyproject.toml\"]\nlocks = [\"web/pnpm-lock.yaml\"]\nscopes = [\"web\"]\n",
            true,
        );
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--check"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("must be named package.json"), "{err}");
        assert!(runner.calls.borrow().is_empty());
    }
}
