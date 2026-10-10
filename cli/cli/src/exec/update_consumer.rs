use super::super::common::{frozen_summary, offline_summary, pre_exec, Env};
use super::{emit_dry_run, record_named, RunMode};
use crate::args::Invocation;
use dx_adopt::dependency_sets::{self, ResolvedSet, ResolvedTarget, Selection};
use std::collections::BTreeMap;

pub(super) fn execute_configured(
    invocation: &Invocation,
    env: Env<'_>,
    registry: &dependency_sets::Registry,
    verbose: bool,
) -> i32 {
    let mode = if invocation.dry_run {
        if invocation.check {
            RunMode::Check
        } else {
            RunMode::Update
        }
    } else if invocation.applies() {
        RunMode::Update
    } else {
        RunMode::Check
    };
    let mode_text = match mode {
        RunMode::Update => "default",
        RunMode::Check => "check",
    };
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let resolved = match dependency_sets::resolve(registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let summary = frozen_summary(
        offline_summary(display_summary(&resolved, mode), invocation.offline),
        invocation.frozen,
    );
    if invocation.dry_run {
        return emit_dry_run(
            invocation,
            out,
            &summary,
            verbose,
            mode_text,
            plans(&resolved, mode, invocation.offline, invocation.frozen),
        );
    }
    if invocation.output == dx_output::OutputMode::Json {
        if let Ok(event) = dx_output::command_started(invocation.command.name(), false, mode_text) {
            let _ = dx_output::write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<String, String> = BTreeMap::new();
    for target in &resolved {
        run_configured_target(
            &mut attempted,
            &mut details,
            target,
            runner,
            workspace,
            mode,
            invocation,
        );
    }
    let selected: Vec<String> = resolved
        .iter()
        .map(|target| target.set.name.clone())
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
    })
}

fn run_configured_target(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<String, String>,
    target: &ResolvedTarget,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    mode: RunMode,
    invocation: &Invocation,
) {
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
            return;
        }
        if let Err(error) = dependency_sets::check_files(workspace, set, false) {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Failed,
                format!("failed to {verb} {}: {error}", set.name),
            );
            return;
        }
    } else if let Err(error) = dependency_sets::check_files(workspace, set, true) {
        record_named(
            attempted,
            details,
            &set.name,
            dx_update::outcome::SetStatus::Failed,
            format!("failed to {verb} {}: {error}", set.name),
        );
        return;
    }
    let planned = match mode {
        RunMode::Update => dx_update::backend::plan_configured(
            set,
            &target.request,
            invocation.offline,
            invocation.frozen,
        )
        .map(|plan| match plan {
            dx_update::backend::BackendPlan::Run { argv, env } => (argv, env),
            dx_update::backend::BackendPlan::Noop => (Vec::new(), Vec::new()),
        }),
        RunMode::Check => dx_update::backend::check_configured(
            set,
            &target.request,
            invocation.offline,
            invocation.frozen,
        )
        .map(|plan| match plan {
            dx_update::backend::CheckPlan::Run { argv, env } => (argv, env),
            dx_update::backend::CheckPlan::Pinned => (Vec::new(), Vec::new()),
            dx_update::backend::CheckPlan::Unavailable => (Vec::new(), Vec::new()),
        }),
    };
    let (argv, extra) = match planned {
        Err(error) => {
            let message = format!("cannot {verb} {}: {error}", set.name);
            let status = match error {
                dx_update::backend::BackendError::Unsupported { .. }
                | dx_update::backend::BackendError::UnsupportedOwned { .. } => {
                    dx_update::outcome::SetStatus::Unsupported
                }
                dx_update::backend::BackendError::OfflineRequired { .. }
                | dx_update::backend::BackendError::OfflineRequiredOwned { .. } => {
                    dx_update::outcome::SetStatus::Failed
                }
                dx_update::backend::BackendError::FrozenLocked { .. }
                | dx_update::backend::BackendError::FrozenLockedOwned { .. } => {
                    dx_update::outcome::SetStatus::Failed
                }
            };
            let message = match status {
                dx_update::outcome::SetStatus::Failed => {
                    format!("failed to {verb} {}: {error}", set.name)
                }
                _ => message,
            };
            record_named(attempted, details, &set.name, status, message);
            return;
        }
        Ok((argv, extra)) => (argv, extra),
    };
    if argv.is_empty() {
        record_named(
            attempted,
            details,
            &set.name,
            dx_update::outcome::SetStatus::Pinned,
            pinned_line(set),
        );
        return;
    }
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    match runner.run(&argv, workspace, &env_refs) {
        Err(error) => {
            record_named(
                attempted,
                details,
                &set.name,
                dx_update::outcome::SetStatus::Failed,
                format!(
                    "failed to {verb} {}: failed to launch updater: {error}",
                    set.name
                ),
            );
        }
        Ok(status) => match status.code {
            Some(0) => {
                let (status, message) = match mode {
                    RunMode::Update => (
                        dx_update::outcome::SetStatus::Updated,
                        success_line(set, &target.request),
                    ),
                    RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
                };
                record_named(attempted, details, &set.name, status, message);
            }
            Some(code) => {
                record_named(
                    attempted,
                    details,
                    &set.name,
                    dx_update::outcome::SetStatus::Failed,
                    format!("failed to {verb} {}: updater exited {code}", set.name),
                );
            }
            None => {
                record_named(
                    attempted,
                    details,
                    &set.name,
                    dx_update::outcome::SetStatus::Failed,
                    format!(
                        "failed to {verb} {}: updater terminated by signal",
                        set.name
                    ),
                );
            }
        },
    }
}

fn plans(resolved: &[ResolvedTarget], mode: RunMode, offline: bool, frozen: bool) -> Vec<String> {
    resolved
        .iter()
        .map(|target| describe_plan(&target.set, &target.request, mode, offline, frozen))
        .collect()
}

fn describe_plan(
    set: &ResolvedSet,
    request: &Selection,
    mode: RunMode,
    offline: bool,
    frozen: bool,
) -> String {
    let verb = match mode {
        RunMode::Update => "update",
        RunMode::Check => "check",
    };
    let planned =
        match mode {
            RunMode::Update => dx_update::backend::plan_configured(set, request, offline, frozen)
                .map(|plan| match plan {
                    dx_update::backend::BackendPlan::Run { argv, .. } => Some(argv.join(" ")),
                    dx_update::backend::BackendPlan::Noop => None,
                }),
            RunMode::Check => dx_update::backend::check_configured(set, request, offline, frozen)
                .map(|plan| match plan {
                    dx_update::backend::CheckPlan::Run { argv, .. } => Some(argv.join(" ")),
                    dx_update::backend::CheckPlan::Pinned => None,
                    dx_update::backend::CheckPlan::Unavailable => None,
                }),
        };
    match planned {
        Ok(Some(argv)) => format!("Would {verb} {}: {argv}", set.name),
        Ok(None) => format!(
            "Would leave {} pinned (manual pins; nothing to resolve)",
            set.name
        ),
        Err(error) => format!("Cannot {verb} {}: {error}", set.name),
    }
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
    use super::super::update_live::ScriptRunner;

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
        assert_eq!(runner.calls.borrow().len(), 2);
        assert_eq!(
            runner.calls.borrow()[0],
            vec![
                "uv".to_owned(),
                "lock".to_owned(),
                "--check".to_owned(),
                "--directory".to_owned(),
                "apps/frontend".to_owned(),
            ]
        );
        assert_eq!(
            runner.calls.borrow()[1],
            vec![
                "uv".to_owned(),
                "lock".to_owned(),
                "--check".to_owned(),
                "--directory".to_owned(),
                "services/worker".to_owned(),
            ]
        );
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
        assert_eq!(runner.calls.borrow().len(), 1);
        assert_eq!(
            runner.calls.borrow()[0],
            vec![
                "uv".to_owned(),
                "lock".to_owned(),
                "--directory".to_owned(),
                "services/worker".to_owned(),
            ]
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
    fn consumer_dry_run_plans_without_launching() {
        let harness = consumer_harness("dry-run", DX_TOML, true);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--dry-run", "worker"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Would update worker: uv lock --directory services/worker"),
            "{out}"
        );
        assert_eq!(runner.calls.borrow().len(), 0);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--check", "--dry-run", "worker"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Would check worker: uv lock --check --directory services/worker"),
            "{out}"
        );
        assert_eq!(runner.calls.borrow().len(), 0);
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
}
