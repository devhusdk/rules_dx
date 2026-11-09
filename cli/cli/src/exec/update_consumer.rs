use super::super::common::{
    Env, check_stdout_write, emit_built, frozen_summary, offline_summary, pre_exec,
};
use super::super::pinned_tool::{PinnedTool, PinnedToolError, QueryBazel, resolve_pinned};
use super::super::staged::{StagedError, ToolRun, run_staged_core, write_outputs};
use super::{
    RunMode, SetDetail, SlotGate, record_backend_error, record_named, runner_status,
    unavailable_message,
};
use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, ResolvedTarget, Selection};
use dx_plan::args::Invocation;
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
        query_runner,
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
    } else if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
        return exit;
    }
    let mut outcomes = Outcomes::default();
    let mut gate = SlotGate {
        err: &mut *err,
        events: &mut *out,
        phase: dx_plan::slots::Phase::new(invocation.command),
        busy: None,
    };
    let bazel = QueryBazel {
        query_runner,
        workspace,
        startup: &invocation.bazel_startup_options,
    };
    let mut tools = UvTools::new(runner, &bazel, invocation.offline);
    for target in &resolved {
        if let Err(stop) = run_configured_target(
            &mut outcomes,
            target,
            runner,
            &mut tools,
            workspace,
            invocation,
            &mut gate,
        ) {
            drop(gate);
            return crate::exec::slot_stop(invocation.output, out, err, stop);
        }
    }
    gate.phase.release();
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
        attempted: outcomes.attempted,
        details: outcomes.details,
        verbose,
        mode,
        locks,
    })
}

pub(super) fn uv_closure(
    runner: &dyn dx_process::Runner,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    workspace: &std::path::Path,
    startup: &[String],
    offline: bool,
) -> dx_update::backend::UvClosure {
    let bazel = QueryBazel {
        query_runner,
        workspace,
        startup,
    };
    UvTools::new(runner, &bazel, offline).closure()
}

/// What each set run reports back.
#[derive(Default)]
struct Outcomes {
    attempted: Vec<dx_update::outcome::SetOutcome>,
    details: BTreeMap<String, SetDetail>,
}

/// The stop behind a gate error code: the busy holders when the wait ran out, else the signal.
fn stop_of(gate: &mut SlotGate<'_>, code: i32) -> dx_plan::slots::Stop {
    gate.busy.take().map_or(
        dx_plan::slots::Stop::Signal(code),
        dx_plan::slots::Stop::Busy,
    )
}

fn run_configured_target(
    outcomes: &mut Outcomes,
    target: &ResolvedTarget,
    runner: &dyn dx_process::Runner,
    tools: &mut UvTools<'_>,
    workspace: &std::path::Path,
    invocation: &Invocation,
    gate: &mut SlotGate<'_>,
) -> Result<(), dx_plan::slots::Stop> {
    let Outcomes { attempted, details } = &mut *outcomes;
    let mode = RunMode::of(invocation);
    let set = &target.set;
    let verb = mode.verb();
    if mode == RunMode::Update {
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
            | dependency_sets::DependencySetsError::MissingPinnedLock { .. }
            | dependency_sets::DependencySetsError::UnsupportedMavenHub { .. }
            | dependency_sets::DependencySetsError::UnsupportedCargoHub { .. }
            | dependency_sets::DependencySetsError::UnsupportedNugetLayout { .. }
            | dependency_sets::DependencySetsError::UnsupportedGoLayout { .. } => {
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
    if mode == RunMode::Check
        && let Err(error) = dependency_sets::check_lock_ready(workspace, set)
    {
        record_named(
            attempted,
            details,
            &set.name,
            dx_update::outcome::SetStatus::Unsupported,
            format!("cannot {verb} {}: {error}", set.name),
        );
        return Ok(());
    }
    if let Some(error) = dx_update::backend::host_env_refusal(set, mode == RunMode::Check, &|key| {
        std::env::var_os(key).is_some()
    }) {
        record_backend_error(attempted, details, &error, verb, &set.name);
        return Ok(());
    }
    // An apply the plan rejects anyway never needs the tools, so skip Bazel.
    let rejected = mode == RunMode::Update && (invocation.offline || invocation.frozen);
    let closure = match set.ecosystem {
        Ecosystem::Uv if !rejected => tools.closure(),
        Ecosystem::Npm => dx_update::backend::UvClosure {
            yarn: std::env::var(dx_update::backend::YARN_BIN_VAR).ok(),
            npm: std::env::var(dx_update::backend::NPM_BIN_VAR).ok(),
            ..Default::default()
        },
        Ecosystem::Uv
        | Ecosystem::Bazel
        | Ecosystem::Ruby
        | Ecosystem::PowerShell
        | Ecosystem::Cargo
        | Ecosystem::Maven
        | Ecosystem::Nuget
        | Ecosystem::Go
        | Ecosystem::Cpan
        | Ecosystem::Pub
        | Ecosystem::Conan
        | Ecosystem::Dub
        | Ecosystem::LuaRocks
        | Ecosystem::Julia
        | Ecosystem::Hex
        | Ecosystem::Composer
        | Ecosystem::Opam
        | Ecosystem::SwiftPm
        | Ecosystem::Ansible
        | Ecosystem::Bicep
        | Ecosystem::Cabal => dx_update::backend::UvClosure::default(),
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
            dx_update::backend::BackendPlan::Staged(run) => {
                dx_update::backend::CheckPlan::Staged(run)
            }
            dx_update::backend::BackendPlan::Sequence(runs) => {
                dx_update::backend::CheckPlan::Sequence(runs)
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
    let runs = match planned {
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
        Ok(dx_update::backend::CheckPlan::Staged(run)) => {
            return run_staged(outcomes, target, &run, runner, workspace, mode, gate);
        }
        Ok(dx_update::backend::CheckPlan::Run { argv, env }) => {
            vec![dx_update::backend::Run { argv, env }]
        }
        Ok(dx_update::backend::CheckPlan::Sequence(runs)) => runs,
    };
    let bazel = runs
        .iter()
        .any(|run| dx_update::backend::runs_bazel(&run.argv));
    let guarded = (mode == RunMode::Check && bazel).then(|| CheckGuard::take(workspace, set));
    let mut launched = Ok(dx_process::ChildStatus::exited(0));
    for run in &runs {
        if let Err(code) = gate.before(&run.argv) {
            return Err(stop_of(gate, code));
        }
        launched = launch(gate, runner, workspace, &run.argv, &run.env);
        if !matches!(&launched, Ok(status) if status.code == Some(0)) {
            break;
        }
    }
    let ok = match mode {
        RunMode::Update => (
            dx_update::outcome::SetStatus::Updated,
            success_line(set, &target.request),
        ),
        RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
    };
    let (status, message) = runner_status(launched, verb, &set.name, ok);
    if let Some(wrote) = guarded.and_then(|guard| guard.restore(workspace)) {
        let hint = match set.ecosystem {
            Ecosystem::Cargo => {
                "; declare `lockfile` in crate.from_cargo and list it in the set's locks"
            }
            Ecosystem::Maven => {
                "; the maven.install lock_file must be listed in the set's locks and MODULE.bazel must not change"
            }
            Ecosystem::Nuget => "; paket2bazel must only write into its scratch output folder",
            _ => "",
        };
        record_named(
            attempted,
            details,
            &set.name,
            dx_update::outcome::SetStatus::Failed,
            format!(
                "failed to {verb} {}: check_wrote: {} (restored){hint}",
                set.name,
                wrote.join(", ")
            ),
        );
        if let Some(detail) = details.get_mut(&set.name) {
            detail.failure_code = Some("check_wrote");
        }
        return Ok(());
    }
    record_named(attempted, details, &set.name, status, message);
    Ok(())
}

/// Runs `argv` in the workspace, in a Bazel slot when it is a Bazel command. The caller has
/// already passed `gate.before`.
fn launch(
    gate: &mut SlotGate<'_>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    argv: &[String],
    extra: &[(String, String)],
) -> std::io::Result<dx_process::ChildStatus> {
    let mut env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let bazel = dx_update::backend::runs_bazel(argv);
    let windows_env = if bazel { Vec::new() } else { windows_env() };
    for (key, value) in &windows_env {
        env_refs.push((key, value.as_str()));
    }
    if bazel {
        match gate.phase.slot() {
            Some(slot) => dx_process::Runner::run(&slot.runner(runner), argv, workspace, &env_refs),
            None => runner.run(argv, workspace, &env_refs),
        }
    } else {
        runner.run_hermetic(argv, workspace, &env_refs)
    }
}

/// The host variables a hermetic tool still needs on Windows; none elsewhere.
pub(crate) fn windows_env() -> Vec<(&'static str, String)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    ["SYSTEMROOT", "TEMP", "TMP", "USERPROFILE"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .collect()
}

/// Runs a generator on copies of the set's inputs and compares what it wrote with the
/// declared locks. Check reports drift; apply writes only the declared files that differ.
fn run_staged(
    outcomes: &mut Outcomes,
    target: &ResolvedTarget,
    run: &dx_update::backend::StagedRun,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    mode: RunMode,
    gate: &mut SlotGate<'_>,
) -> Result<(), dx_plan::slots::Stop> {
    use dx_update::outcome::SetStatus;
    let Outcomes { attempted, details } = outcomes;
    let set = &target.set;
    let verb = mode.verb();
    let mut launch_tool = |argv: &[String], env: &[(String, String)]| {
        gate.before(argv)?;
        let guarded = (mode == RunMode::Check).then(|| CheckGuard::take(workspace, set));
        let launched = launch(gate, runner, workspace, argv, env);
        Ok(ToolRun {
            launched,
            wrote: guarded.and_then(|guard| guard.restore(workspace)),
        })
    };
    let differing = match run_staged_core(run, workspace, &set.name, verb, &mut launch_tool) {
        Ok(differing) => differing,
        Err(StagedError::Exit(code)) => return Err(stop_of(gate, code)),
        Err(StagedError::Failed { message, code }) => {
            record_named(attempted, details, &set.name, SetStatus::Failed, message);
            if let Some(code) = code
                && let Some(detail) = details.get_mut(&set.name)
            {
                detail.failure_code = Some(code);
            }
            return Ok(());
        }
    };
    match mode {
        RunMode::Check => {
            if differing.is_empty() {
                record_named(
                    attempted,
                    details,
                    &set.name,
                    SetStatus::Current,
                    current_line(set),
                );
            } else {
                let paths: Vec<&str> = differing.iter().map(|(rel, _)| rel.as_str()).collect();
                record_named(
                    attempted,
                    details,
                    &set.name,
                    SetStatus::Failed,
                    format!(
                        "failed to {verb} {}: stale generated locks: {}; run `dx update {}`",
                        set.name,
                        paths.join(", "),
                        set.name
                    ),
                );
            }
        }
        RunMode::Update => match write_outputs(workspace, differing) {
            Ok(()) => record_named(
                attempted,
                details,
                &set.name,
                SetStatus::Updated,
                success_line(set, &target.request),
            ),
            Err(error) => record_named(
                attempted,
                details,
                &set.name,
                SetStatus::Failed,
                format!("failed to {verb} {}: {error}", set.name),
            ),
        },
    }
    Ok(())
}

/// The bytes of a set's manifests and locks before a Bazel-run check, so a check that writes
/// is caught, undone and failed instead of reported current.
/// `(rel, resolved path before the check, bytes before the check)`.
type GuardedFile = (String, Option<std::path::PathBuf>, Option<Vec<u8>>);

struct CheckGuard {
    files: Vec<GuardedFile>,
}

impl CheckGuard {
    fn take(workspace: &std::path::Path, set: &ResolvedSet) -> CheckGuard {
        let mut paths: Vec<&String> = set.manifests.iter().chain(set.locks.iter()).collect();
        paths.sort();
        paths.dedup();
        CheckGuard {
            files: paths
                .into_iter()
                .map(|rel| {
                    let resolved =
                        dx_path::workspace::resolve_target_in_workspace(workspace, rel).ok();
                    let before = resolved.as_ref().and_then(|path| std::fs::read(path).ok());
                    (rel.clone(), resolved, before)
                })
                .collect(),
        }
    }

    /// Restores every changed file and names them; `None` when the check wrote nothing.
    /// Never writes through a path that was not re-resolved inside the workspace.
    fn restore(self, workspace: &std::path::Path) -> Option<Vec<String>> {
        let mut wrote = Vec::new();
        for (rel, stored, before) in self.files {
            let Some(stored) = stored else {
                continue;
            };
            let now = dx_path::workspace::resolve_target_in_workspace(workspace, &rel);
            if now.as_ref().ok() != Some(&stored) {
                // The check swapped in a link that leaves the workspace or points elsewhere:
                // drop the link itself, then put the original bytes back at the stored path.
                let link = workspace.join(&rel);
                if std::fs::symlink_metadata(&link)
                    .map(|metadata| metadata.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    let _ = std::fs::remove_file(&link);
                }
                if let Some(bytes) = &before {
                    let _ = dx_atomic_fs::write_atomic(&stored, bytes);
                }
                wrote.push(rel);
                continue;
            }
            if std::fs::read(&stored).ok() == before {
                continue;
            }
            let _ = match &before {
                Some(bytes) => dx_atomic_fs::write_atomic(&stored, bytes),
                None => std::fs::remove_file(&stored),
            };
            wrote.push(rel);
        }
        (!wrote.is_empty()).then_some(wrote)
    }
}

/// One resolved tool: its path and, when Bazel was tried and failed, why.
type Resolved = (Option<String>, Option<String>);

/// Resolves uv and its interpreter at most once per run, from the environment
/// first and Bazel second. `PATH` is never searched.
struct UvTools<'a> {
    runner: &'a dyn dx_process::Runner,
    bazel: &'a dyn dx_tool_resolve::Bazel,
    offline: bool,
    uv: Option<Resolved>,
    python: Option<Resolved>,
}

impl<'a> UvTools<'a> {
    fn new(
        runner: &'a dyn dx_process::Runner,
        bazel: &'a dyn dx_tool_resolve::Bazel,
        offline: bool,
    ) -> Self {
        UvTools {
            runner,
            bazel,
            offline,
            uv: None,
            python: None,
        }
    }

    fn resolve(&self, var: &str, label: &str) -> Resolved {
        let tool = PinnedTool { var, label };
        match resolve_pinned(self.runner, self.bazel, &tool, self.offline) {
            Ok(path) => (Some(path.to_string_lossy().into_owned()), None),
            Err(PinnedToolError::Invalid) => (None, None),
            Err(PinnedToolError::Unresolved { detail }) => (None, Some(detail)),
        }
    }

    fn closure(&mut self) -> dx_update::backend::UvClosure {
        use dx_update::backend as be;
        let runner = self.runner;
        let (uv, uv_detail) = match self.uv.take() {
            Some(done) => done,
            None => {
                let (path, detail) = self.resolve(be::UV_BIN_VAR, be::UV_LABEL);
                let path = path.filter(|path| {
                    runner
                        .tool_version(std::path::Path::new(path))
                        .is_some_and(|line| {
                            let name_and_version: Vec<&str> =
                                line.split_whitespace().take(2).collect();
                            name_and_version.join(" ") == be::UV_VERSION_LINE
                        })
                });
                (path, detail)
            }
        };
        self.uv = Some((uv.clone(), uv_detail.clone()));
        // A missing uv already fails the set, so skip resolving the interpreter.
        let (python, python_detail) = if uv.is_some() {
            let done = match self.python.take() {
                Some(done) => done,
                None => self.resolve(be::UV_PYTHON_VAR, be::UV_PYTHON_LABEL),
            };
            self.python = Some(done.clone());
            done
        } else {
            (None, None)
        };
        be::UvClosure {
            uv,
            python,
            uv_detail,
            python_detail,
            ..Default::default()
        }
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
    #[test]
    fn stop_of_reports_the_holder_once_then_the_signal() {
        let (mut err, mut events) = (Vec::new(), Vec::new());
        let mut gate = super::SlotGate {
            err: &mut err,
            events: &mut events,
            phase: dx_plan::slots::Phase::new(dx_plan::args::Command::Update),
            busy: Some("pid 7".to_owned()),
        };
        assert_eq!(
            super::stop_of(&mut gate, 1),
            dx_plan::slots::Stop::Busy("pid 7".to_owned())
        );
        assert_eq!(
            super::stop_of(&mut gate, 130),
            dx_plan::slots::Stop::Signal(130)
        );
    }

    use super::super::super::test_support::Harness;
    use super::super::super::test_support::{event, event_kinds, json_events};
    use super::super::update_live::{Launch, ScriptRunner, fake_pinned};

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

    const UV_MODULE: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"deps\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\nuv.project(hub_name = \"deps\", pyproject = \"//services/worker:pyproject.toml\", lock = \"//services/worker:uv.lock\")\n";

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

    fn consumer_harness(name: &str, module: &str, with_locks: bool) -> Harness {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let harness = Harness::new(&format!("update-consumer-{name}-{id}"));
        harness.write_source("MODULE.bazel", module);
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
        let invocation = dx_plan::args::parse(&words).expect("parse");
        harness.execute_with(&invocation, runner)
    }

    fn run_consumer(argv: &[&str], runner: &ScriptRunner) -> (i32, String, String, Harness) {
        let harness = consumer_harness("sets", UV_MODULE, true);
        let (code, out, err) = run_with(argv, runner, &harness);
        (code, out, err, harness)
    }

    #[test]
    fn consumer_bare_check_runs_configured_sets_and_no_phantoms() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Running update --check for deps-frontend, deps-worker"),
            "{out}"
        );
        assert!(
            out.contains("deps-frontend lockfile current (apps/frontend/uv.lock)"),
            "{out}"
        );
        assert!(
            out.contains("deps-worker lockfile current (services/worker/uv.lock)"),
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
        let (code, out, err, _) = run_consumer(&["update"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(err.contains("DX_UV_BIN"), "{err}");
        assert!(err.contains("PATH is never searched"), "{err}");
        assert!(!out.contains("lockfile current"), "{out}");
        assert!(runner.calls.borrow().is_empty());
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let (code, out, err, _) = run_consumer(&["update", "--apply", "deps-worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(!out.contains("updated deps-worker"), "{out}");
        assert!(runner.calls.borrow().is_empty());
    }

    /// Scripts one Bazel resolution of `rel` under a fresh output base.
    fn script_resolution(harness: &Harness, base: &std::path::Path, rel: &str) {
        let file = base.join(rel);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
        std::fs::write(&file, b"").expect("tool");
        harness
            .query
            .outputs
            .borrow_mut()
            .push(dx_plan::resolve::QueryResult {
                code: Some(0),
                stdout: format!("{rel}\n").into_bytes(),
                stderr: Vec::new(),
            });
        harness.query.script_info(&base.to_string_lossy());
    }

    #[test]
    fn consumer_unset_uv_resolves_through_bazel_once_for_every_set() {
        let base = super::super::super::test_support::temp_dir("uv-fallback");
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let harness = consumer_harness("fallback", UV_MODULE, true);
        script_resolution(&harness, base.path(), "external/x/uv");
        script_resolution(&harness, base.path(), "external/x/python3");
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let calls = harness.query.calls.borrow();
        assert_eq!(calls.len(), 2, "one cquery per tool: {calls:?}");
        assert!(calls[0].contains(&"@dx_tools//:uv".to_owned()), "{calls:?}");
        assert!(
            !calls[0]
                .iter()
                .any(|arg| arg.contains("repository_disable"))
        );
        let launched = runner.calls.borrow();
        assert_eq!(launched.len(), 2, "{launched:?}");
        let uv = base.path().join("external").join("x").join("uv");
        assert_eq!(launched[0][0], uv.to_string_lossy());
        assert_eq!(launched[1][0], uv.to_string_lossy());
    }

    #[test]
    fn consumer_offline_check_disables_repository_downloads_for_resolution() {
        let base = super::super::super::test_support::temp_dir("uv-offline");
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let harness = consumer_harness("offline", UV_MODULE, true);
        script_resolution(&harness, base.path(), "external/x/uv");
        script_resolution(&harness, base.path(), "external/x/python3");
        let (code, out, err) = run_with(&["update", "--offline"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let calls = harness.query.calls.borrow();
        assert!(!calls.is_empty());
        for call in calls.iter() {
            assert!(
                call.contains(&"--repository_disable_download".to_owned()),
                "{call:?}"
            );
            assert!(!call.contains(&"--nofetch".to_owned()), "{call:?}");
        }
    }

    #[test]
    fn consumer_failed_resolution_is_unavailable_with_the_detail_and_queried_once() {
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let harness = consumer_harness("resolve-fails", UV_MODULE, true);
        harness
            .query
            .script_failure(1, "ERROR: no such repository '@dx_tools'\n");
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(err.contains("DX_UV_BIN"), "{err}");
        assert!(err.contains("PATH is never searched"), "{err}");
        assert!(err.contains("no such repository"), "{err}");
        assert_eq!(harness.query.calls.borrow().len(), 1);
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn consumer_resolved_uv_with_the_wrong_version_is_rejected() {
        let base = super::super::super::test_support::temp_dir("uv-wrong-version");
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        runner.version = Some("uv 0.1.0".to_owned());
        let harness = consumer_harness("wrong-version", UV_MODULE, true);
        script_resolution(&harness, base.path(), "external/x/uv");
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("backend_unavailable"), "{err}");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn consumer_rejected_applies_never_query_bazel() {
        for flag in ["--offline", "--frozen"] {
            let mut runner = ScriptRunner::new(&[]);
            runner.pinned = false;
            let harness = consumer_harness("rejected", UV_MODULE, true);
            let (_, out, err) = run_with(
                &["update", "--apply", flag, "deps-worker"],
                &runner,
                &harness,
            );
            assert!(
                harness.query.calls.borrow().is_empty(),
                "{flag}: {out}{err}"
            );
            assert!(harness.query.info_calls.borrow().is_empty());
        }
    }

    #[test]
    fn consumer_backend_unavailable_reports_its_code() {
        let mut runner = ScriptRunner::new(&[]);
        runner.pinned = false;
        let (code, out, err, _) = run_consumer(&["update", "--output=json"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("\"code\":\"backend_unavailable\""), "{out}");
        assert!(!out.contains("\"code\":\"update_failed\""), "{out}");
        for args in [&["update"][..], &["update", "--apply", "deps-worker"][..]] {
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
            let (code, out, err, _) = run_consumer(&["update"], &runner);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains("backend_unavailable"), "{err}");
            assert!(err.contains("uv 0.13.7"), "{err}");
            assert!(!out.contains("lockfile current"), "{out}");
            assert!(runner.calls.borrow().is_empty());
        }
    }

    #[test]
    fn consumer_failed_uv_fails_its_set_and_keeps_the_others() {
        let runner = ScriptRunner::new(&[("consumer-uv:apps/frontend", Some(1))]);
        let (code, out, err, _) = run_consumer(&["update"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to check deps-frontend: updater exited 1"),
            "{err}"
        );
        assert!(
            out.contains("deps-worker lockfile current (services/worker/uv.lock)"),
            "{out}"
        );
        assert!(!out.contains("deps-frontend lockfile current"), "{out}");
        assert_eq!(runner.calls.borrow().len(), 2);
        let runner = ScriptRunner::new(&[("consumer-uv:services/worker", None)]);
        let (code, out, err, _) = run_consumer(&["update", "--apply", "deps-worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to update deps-worker: updater terminated by signal"),
            "{err}"
        );
        let mut runner = ScriptRunner::new(&[]);
        runner.io_error = true;
        let (code, out, err, _) = run_consumer(&["update", "deps-worker"], &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to launch updater"), "{err}");
    }

    #[test]
    fn consumer_uv_settings_outside_the_closure_are_unsupported() {
        let harness = consumer_harness("settings", UV_MODULE, true);
        harness.write_source(
            "apps/frontend/uv.toml",
            "exclude-newer = \"2020-01-01T00:00:00Z\"\n",
        );
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("apps/frontend/uv.toml exists"), "{err}");
        assert!(
            out.contains("deps-worker lockfile current (services/worker/uv.lock)"),
            "{out}"
        );
        assert_eq!(
            *runner.calls.borrow(),
            vec![uv_call(true, "services/worker")]
        );
        let harness = consumer_harness("tool-uv", UV_MODULE, true);
        harness.write_source(
            "services/worker/pyproject.toml",
            &format!("{PYPROJECT}\n[tool.uv]\nindex-url = \"https://example.invalid/simple\"\n"),
        );
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "deps-worker"], &runner, &harness);
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
        assert!(err.contains("dx update --apply deps-worker"), "{err}");
        assert!(
            err.contains("git checkout -- apps/frontend/uv.lock"),
            "{err}"
        );
        assert!(!err.contains("nothing to roll back"), "{err}");
    }

    #[test]
    fn consumer_selective_update_runs_one_directory() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--apply", "deps-worker"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running update for deps-worker"), "{out}");
        assert!(
            out.contains("updated deps-worker (services/worker/uv.lock)"),
            "{out}"
        );
        assert!(!out.contains("deps-frontend"), "{out}");
        assert_eq!(err, "", "{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![uv_call(false, "services/worker")]
        );
    }

    #[test]
    fn consumer_scopes_resolve_to_their_owning_sets() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "apps/frontend/app/main.py"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 1);
        assert!(runner.calls.borrow()[0].contains(&"apps/frontend".to_owned()));
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "//..."], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.calls.borrow().len(), 2);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "//services/worker/..."], &runner);
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
        let (code, _, err, _) =
            run_consumer(&["update", "--apply", "deps-frontend:anyio"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("deps-frontend"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_missing_lock_fails_check_without_launch() {
        let harness = consumer_harness("missing-lock", UV_MODULE, false);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "deps-worker"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
        assert!(err.contains("is missing"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_check_leaves_the_workspace_untouched() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, _, harness) = run_consumer(&["update"], &runner);
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
            std::fs::read(harness.workspace.join("MODULE.bazel")).expect("module"),
            UV_MODULE.as_bytes()
        );
    }

    #[test]
    fn consumer_offline_update_needs_network() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) =
            run_consumer(&["update", "--apply", "--offline", "deps-worker"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_frozen_update_locks_resolution() {
        let runner = ScriptRunner::new(&[]);
        let (code, _, err, _) =
            run_consumer(&["update", "--apply", "--frozen", "deps-worker"], &runner);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        assert!(!err.contains("offline_required"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--frozen", "deps-worker"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("deps-worker lockfile current (services/worker/uv.lock)"),
            "{out}{err}"
        );
    }

    #[test]
    fn consumer_invalid_config_fails_before_any_backend() {
        let harness = consumer_harness(
            "invalid",
            "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"deps\")\n",
            true,
        );
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("uv.project"), "{err}");
        assert_eq!(runner.calls.borrow().len(), 0);
    }

    #[test]
    fn consumer_json_check_reports_per_set_correlations() {
        let runner = ScriptRunner::new(&[]);
        let (code, out, err, _) = run_consumer(&["update", "--output=json"], &runner);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(
            out.contains("\"correlation\":\"update:deps-frontend\""),
            "{out}"
        );
        assert!(
            out.contains("\"correlation\":\"update:deps-worker\""),
            "{out}"
        );
        let _ = event(&events, "command_started");
    }

    const MIXED_MODULE: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"deps\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"jobs\", pnpm_lock = \"//consumer-npm/jobs:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"web\", pnpm_lock = \"//consumer-npm/web:pnpm-lock.yaml\")\n";

    const PACKAGE_JSON: &str = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\"\n}\n";

    const PNPM_LOCK: &str = "lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n";

    fn mixed_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, MIXED_MODULE, true);
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

    #[allow(clippy::disallowed_methods)] // test helper mirrors the backend argv
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
            &["--config=dx_build_dev".to_owned()],
        ));
        argv.push("--config=dx_build_dev".to_owned());
        argv.extend(dx_plan::plan::shared_settings("run"));
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
        let (code, out, err) = run_with(&["update"], &runner, &harness);
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
        assert!(
            !harness
                .workspace
                .join("consumer-npm/web/node_modules")
                .exists()
        );
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
        let gate =
            dx_plan::slots::Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), false);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = dx_plan::slots::with_gate(Some(gate), || {
            run_with(&["update", "web"], &runner, &harness)
        });
        assert_eq!(code, 0, "{out}{err}");
        let marked = (dx_plan::slots::SLOT_ENV_VAR.to_owned(), "1".to_owned());
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
        let (code, out, err) = run_with(&["update"], &runner, &harness);
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
            out.contains("deps-frontend lockfile current (apps/frontend/uv.lock)"),
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
        let (code, _, err) = run_with(&["update", "web:react"], &runner, &harness);
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
        let (code, out, err) = run_with(&["update"], &runner, &harness);
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

    const PINNED_MODULE: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"deps\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\nruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"gems\", gemfile = \"//apps/rb:Gemfile\", gemfile_lock = \"//apps/rb:Gemfile.lock\")\n";

    const PINNED_FILES: [(&str, &str); 2] = [
        (
            "apps/rb/Gemfile",
            "source \"https://rubygems.org\"\ngem \"rspec-core\"\n",
        ),
        (
            "apps/rb/Gemfile.lock",
            "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec-core (3.13.0)\n",
        ),
    ];

    fn pinned_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, PINNED_MODULE, true);
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
    fn consumer_ruby_set_reports_pinned_and_launches_nothing() {
        for (argv, uv) in [
            (vec!["update"], uv_call(true, "apps/frontend")),
            (
                vec!["update", "--offline", "--frozen"],
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
            assert!(
                events
                    .iter()
                    .any(|event| event["code"] == "update_set_pinned"
                        && event["correlation"] == "update:gems"),
                "{argv:?}: {out}"
            );
            assert!(!out.contains("gems lockfile current"), "{out}");
            assert!(!out.contains("updated gems"), "{out}");
            assert_pinned_files_unchanged(&harness);
        }
        let harness = pinned_harness("pinned-text");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("gems pins are manual; nothing to resolve"),
            "{out}"
        );
    }

    #[test]
    fn consumer_pinned_package_selection_is_unsupported_without_fallback() {
        for mode in ["--apply", "--output=text"] {
            let harness = pinned_harness("pinned-selective");
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) =
                run_with(&["update", mode, "gems:rspec-core"], &runner, &harness);
            assert_ne!(code, 0, "{mode}: {out}{err}");
            assert!(err.contains("update_set_unsupported"), "{err}");
            assert!(err.contains("bundle lock"), "{err}");
            assert!(!out.contains("pins are manual"), "{out}");
            assert!(runner.launches.borrow().is_empty());
            assert_pinned_files_unchanged(&harness);
        }
    }

    #[test]
    fn consumer_pinned_set_without_its_lock_declared_is_unsupported() {
        let harness = consumer_harness(
            "pinned-lock",
            "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"gems\", gemfile = \"//apps/rb:Gemfile\", gemfile_lock = \"//apps/rb:gems.locked\")\n",
            true,
        );
        harness.write_source("apps/rb/Gemfile", PINNED_FILES[0].1);
        harness.write_source("apps/rb/gems.locked", PINNED_FILES[1].1);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("apps/rb/Gemfile.lock"), "{err}");
        assert!(!out.contains("pins are manual"), "{out}");
        assert!(runner.launches.borrow().is_empty());
    }

    const CARGO_MANIFEST: &str =
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

    const CARGO_LOCK: &str = "version = 4\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

    const CARGO_FILES: [&str; 6] = [
        "consumer-rs/crates/Cargo.toml",
        "consumer-rs/crates/Cargo.lock",
        "consumer-rs/crates/cargo-bazel-lock.json",
        "consumer-rs/tools/Cargo.toml",
        "consumer-rs/tools/Cargo.lock",
        "consumer-rs/tools/cargo-bazel-lock.json",
    ];

    const CARGO_MODULE: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//consumer-rs/crates:Cargo.lock\", lockfile = \"//consumer-rs/crates:cargo-bazel-lock.json\", manifests = [\"//consumer-rs/crates:Cargo.toml\"])\ncrate.from_cargo(name = \"tool_crates\", cargo_lockfile = \"//consumer-rs/tools:Cargo.lock\", lockfile = \"//consumer-rs/tools:cargo-bazel-lock.json\", manifests = [\"//consumer-rs/tools:Cargo.toml\"])\nuse_repo(crate, \"crates\", \"tool_crates\")\n";

    fn cargo_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, &format!("{MIXED_MODULE}{CARGO_MODULE}"), true);
        for dir in ["consumer-npm/jobs", "consumer-npm/web"] {
            harness.write_source(&format!("{dir}/package.json"), PACKAGE_JSON);
            harness.write_source(&format!("{dir}/pnpm-lock.yaml"), PNPM_LOCK);
        }
        for dir in ["consumer-rs/crates", "consumer-rs/tools"] {
            harness.write_source(&format!("{dir}/Cargo.toml"), CARGO_MANIFEST);
            harness.write_source(&format!("{dir}/Cargo.lock"), CARGO_LOCK);
            harness.write_source(&format!("{dir}/cargo-bazel-lock.json"), "{}\n");
        }
        harness
    }

    #[allow(clippy::disallowed_methods)] // test helper mirrors the backend argv
    fn cargo_call(startup: &[String], offline: bool, repository: &str) -> Vec<String> {
        let mut argv = dx_process::startup_argv(startup);
        argv.push("fetch".to_owned());
        argv.extend(dx_process::build_verb_options(
            "fetch",
            &["--config=dx_build_dev".to_owned()],
        ));
        argv.push("--config=dx_build_dev".to_owned());
        if offline {
            argv.push("--repository_disable_download".to_owned());
        }
        argv.push(format!("--repo=@{repository}"));
        argv
    }

    fn cargo_env(repin: &str, repository: &str) -> Vec<(String, String)> {
        dx_update::backend::cargo_env(repin, repository)
    }

    fn cargo_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        CARGO_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    #[test]
    fn consumer_cargo_env_forces_both_repin_triggers() {
        assert_eq!(
            cargo_env("false", "crates"),
            vec![
                ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
                ("CARGO_BAZEL_REPIN".to_owned(), "false".to_owned()),
                ("CARGO_BAZEL_REPIN_ONLY".to_owned(), "crates".to_owned()),
                ("REPIN".to_owned(), "false".to_owned()),
                ("CARGO_BAZEL_DIGEST_CHECK".to_owned(), "1".to_owned()),
            ]
        );
    }

    #[test]
    fn consumer_cargo_check_env_checks_the_lock_digest_but_apply_does_not() {
        let has =
            |env: Vec<(String, String)>| env.iter().any(|(k, _)| k == "CARGO_BAZEL_DIGEST_CHECK");
        assert!(has(cargo_env("false", "crates")));
        assert!(!has(cargo_env("workspace", "crates")));
        assert!(!has(cargo_env("serde", "crates")));
    }

    #[test]
    fn consumer_mixed_check_runs_uv_npm_and_cargo_each_their_own_way() {
        let harness = cargo_harness("cargo-check");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("crates lockfile current (consumer-rs/crates/Cargo.lock, consumer-rs/crates/cargo-bazel-lock.json)"),
            "{out}"
        );
        assert!(
            out.contains("web lockfile current (consumer-npm/web/pnpm-lock.yaml)"),
            "{out}"
        );
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 5, "{launches:?}");
        assert_eq!(launches[1].argv, uv_call(true, "apps/frontend"));
        assert!(launches[1].hermetic);
        assert_eq!(
            launches[0],
            Launch {
                argv: cargo_call(&[], false, "crates"),
                cwd: harness.workspace.clone(),
                env: cargo_env("false", "crates"),
                hermetic: false,
            }
        );
        assert_eq!(launches[2].argv, npm_check(&harness, "consumer-npm/jobs"));
        assert_eq!(launches[2].env, npm_env());
        assert_eq!(
            launches[3],
            Launch {
                argv: cargo_call(&[], false, "tool_crates"),
                cwd: harness.workspace.clone(),
                env: cargo_env("false", "tool_crates"),
                hermetic: false,
            }
        );
        assert_eq!(launches[4].argv, npm_check(&harness, "consumer-npm/web"));
    }

    #[test]
    fn consumer_cargo_apply_repins_one_set_and_leaves_the_others() {
        let harness = cargo_harness("cargo-apply");
        let others = [
            "consumer-rs/tools/Cargo.lock",
            "consumer-rs/tools/cargo-bazel-lock.json",
            "consumer-npm/web/pnpm-lock.yaml",
            "apps/frontend/uv.lock",
        ];
        let before: Vec<Vec<u8>> = others.iter().map(|rel| read_bytes(&harness, rel)).collect();
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "crates"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("updated crates (consumer-rs/crates/Cargo.lock, consumer-rs/crates/cargo-bazel-lock.json)"),
            "{out}"
        );
        assert_eq!(
            *runner.launches.borrow(),
            vec![Launch {
                argv: cargo_call(&[], false, "crates"),
                cwd: harness.workspace.clone(),
                env: cargo_env("workspace", "crates"),
                hermetic: false,
            }]
        );
        let after: Vec<Vec<u8>> = others.iter().map(|rel| read_bytes(&harness, rel)).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn consumer_cargo_apply_package_repins_one_crate() {
        let harness = cargo_harness("cargo-apply-package");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "crates:serde"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("updated crates:serde (consumer-rs/crates/Cargo.lock, consumer-rs/crates/cargo-bazel-lock.json)"),
            "{out}"
        );
        assert_eq!(
            *runner.launches.borrow(),
            vec![Launch {
                argv: cargo_call(&[], false, "crates"),
                cwd: harness.workspace.clone(),
                env: cargo_env("serde", "crates"),
                hermetic: false,
            }]
        );
    }

    #[test]
    fn consumer_cargo_package_and_full_sets_in_one_run() {
        let harness = cargo_harness("cargo-package-and-full");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "crates:serde", "tool_crates"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 2);
        assert_eq!(launches[0].argv, cargo_call(&[], false, "crates"));
        assert_eq!(launches[0].env, cargo_env("serde", "crates"));
        assert_eq!(launches[1].argv, cargo_call(&[], false, "tool_crates"));
        assert_eq!(launches[1].env, cargo_env("workspace", "tool_crates"));
    }

    #[test]
    fn consumer_cargo_package_failure_stays_with_its_own_set() {
        let harness = cargo_harness("cargo-package-fail");
        let runner = ScriptRunner::new(&[("consumer-cargo:crates", Some(1))]);
        let (code, out, err) = run_with(
            &["update", "--apply", "crates:serde", "tool_crates"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("updater exited 1"), "{err}");
        assert!(out.contains("updated tool_crates"), "{out}");
        assert_eq!(runner.calls.borrow().len(), 2);
    }

    #[test]
    fn consumer_cargo_failure_stays_with_its_own_set() {
        let harness = cargo_harness("cargo-fail");
        let runner = ScriptRunner::new(&[("consumer-cargo:tool_crates", Some(1))]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to check tool_crates: updater exited 1"),
            "{err}"
        );
        assert!(!out.contains("tool_crates lockfile current"), "{out}");
        assert!(out.contains("crates lockfile current"), "{out}");
        assert!(out.contains("web lockfile current"), "{out}");
        assert_eq!(runner.calls.borrow().len(), 5);
    }

    #[test]
    fn consumer_cargo_refusals_never_launch_bazel() {
        let harness = cargo_harness("cargo-refuse");
        let before = cargo_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(
            &["update", "--apply", "--frozen", "crates"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--offline", "crates"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, out, err) = run_with(&["update", "crates:serde"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("select the set without a package"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--frozen", "crates:serde"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--offline", "crates:serde"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "crates:serde", "crates:tokio"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(err.contains("one crate per run"), "{err}");
        for name in ["off", "all"] {
            let (code, _, err) = run_with(
                &["update", "--apply", &format!("crates:{name}")],
                &runner,
                &harness,
            );
            assert_eq!(code, 1, "{name}: {err}");
            assert!(err.contains("update_set_unsupported"), "{err}");
            assert!(err.contains("repin mode"), "{err}");
        }
        assert!(runner.calls.borrow().is_empty());
        assert_eq!(cargo_bytes(&harness), before);
    }

    #[test]
    fn consumer_cargo_offline_check_disables_downloads() {
        let harness = cargo_harness("cargo-offline");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &[
                "update",
                "--offline",
                "--bazel-startup-option=--output_base=/tmp/cargo-ob",
                "crates",
            ],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            *runner.calls.borrow(),
            vec![cargo_call(
                &["--output_base=/tmp/cargo-ob".to_owned()],
                true,
                "crates"
            )]
        );
    }

    #[test]
    fn consumer_bazel_check_that_writes_fails_and_is_restored() {
        let harness = cargo_harness("cargo-guard");
        let before = cargo_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        runner.writes.borrow_mut().push((
            harness.workspace.join("consumer-rs/crates/Cargo.lock"),
            b"rewritten\n".to_vec(),
        ));
        let (code, out, err) = run_with(&["update", "crates"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("failed to check crates: check_wrote: consumer-rs/crates/Cargo.lock (restored); declare `lockfile` in crate.from_cargo"),
            "{err}"
        );
        assert!(!out.contains("crates lockfile current"), "{out}");
        assert_eq!(cargo_bytes(&harness), before);
        let runner = ScriptRunner::new(&[]);
        runner.writes.borrow_mut().push((
            harness.workspace.join("consumer-npm/web/pnpm-lock.yaml"),
            b"rewritten\n".to_vec(),
        ));
        let (code, out, err) = run_with(&["update", "web"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(
                "failed to check web: check_wrote: consumer-npm/web/pnpm-lock.yaml (restored)"
            ),
            "{err}"
        );
        assert!(!err.contains("crate.from_cargo"), "{err}");
        assert_eq!(
            read_bytes(&harness, "consumer-npm/web/pnpm-lock.yaml"),
            PNPM_LOCK.as_bytes()
        );
    }

    const MAVEN_MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"tool_maven\", lock_file = \"//tools/jvm:tool_install.json\")\nuse_repo(maven, \"maven\", \"tool_maven\")\n";

    const MAVEN_LOCK: &str = "{\"__AUTOGENERATED_FILE_DO_NOT_MODIFY_THIS_FILE_MANUALLY\":\"x\",\"__INPUT_ARTIFACTS_HASH\":1,\"__RESOLVED_ARTIFACTS_HASH\":2,\"artifacts\":{}}\n";

    const MAVEN_FILES: [&str; 3] = [
        "MODULE.bazel",
        "third_party/maven_install.json",
        "tools/jvm/tool_install.json",
    ];

    fn maven_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, &format!("{UV_MODULE}{MAVEN_MODULE}"), true);
        harness.write_source("third_party/maven_install.json", MAVEN_LOCK);
        harness.write_source("tools/jvm/tool_install.json", MAVEN_LOCK);
        harness
    }

    #[allow(clippy::disallowed_methods)] // test helper mirrors the backend argv
    fn maven_pin_call(repository: &str) -> Vec<String> {
        let mut argv = dx_process::startup_argv(&[]);
        argv.push("run".to_owned());
        argv.push("--config=dx_build_dev".to_owned());
        argv.push(format!("@{repository}//:pin"));
        argv
    }

    fn maven_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        MAVEN_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    #[test]
    fn consumer_maven_check_fetches_each_hub_without_repin_env() {
        let harness = maven_harness("maven-check");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "maven", "tool_maven"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("maven lockfile current (third_party/maven_install.json)"),
            "{out}"
        );
        let launches = runner.launches.borrow();
        assert_eq!(
            *launches,
            vec![
                Launch {
                    argv: cargo_call(&[], false, "maven"),
                    cwd: harness.workspace.clone(),
                    env: Vec::new(),
                    hermetic: false,
                },
                Launch {
                    argv: cargo_call(&[], false, "tool_maven"),
                    cwd: harness.workspace.clone(),
                    env: Vec::new(),
                    hermetic: false,
                },
            ]
        );
    }

    #[test]
    fn consumer_maven_apply_pins_one_set_and_leaves_the_other_lock() {
        let harness = maven_harness("maven-apply");
        let before = maven_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        runner.writes.borrow_mut().push((
            harness.workspace.join("third_party/maven_install.json"),
            b"{\"pinned\":true}\n".to_vec(),
        ));
        let (code, out, err) = run_with(&["update", "--apply", "maven"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("updated maven"), "{out}");
        assert_eq!(
            *runner.launches.borrow(),
            vec![Launch {
                argv: maven_pin_call("maven"),
                cwd: harness.workspace.clone(),
                env: vec![("RULES_JVM_EXTERNAL_REPIN".to_owned(), "1".to_owned())],
                hermetic: false,
            }]
        );
        let after = maven_bytes(&harness);
        assert_eq!(after[0], before[0]);
        assert_ne!(after[1], before[1]);
        assert_eq!(after[2], before[2]);
    }

    #[test]
    fn consumer_maven_refusals_never_launch_bazel() {
        let harness = maven_harness("maven-refusals");
        for argv in [
            vec!["update", "--apply", "maven:com.google.guava:guava"],
            vec!["update", "--apply", "--offline", "maven"],
            vec!["update", "maven:com.google.guava:guava"],
        ] {
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{argv:?}: {out}{err}");
            assert!(runner.launches.borrow().is_empty(), "{argv:?}");
        }
        let runner = ScriptRunner::new(&[]);
        let (_, out, err) = run_with(
            &["update", "--apply", "maven:com.google.guava:guava"],
            &runner,
            &harness,
        );
        assert!(
            format!("{out}{err}").contains("maven pins are exact in MODULE.bazel"),
            "{out}{err}"
        );
    }

    #[test]
    fn consumer_maven_check_needs_the_hub_and_a_signed_lock() {
        for (name, rewrite) in [
            ("empty", ("third_party/maven_install.json", "")),
            (
                "v1",
                ("third_party/maven_install.json", "{\"dependency_tree\":{}}"),
            ),
        ] {
            let harness = maven_harness(&format!("maven-unready-{name}"));
            harness.write_source(rewrite.0, rewrite.1);
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&["update", "maven"], &runner, &harness);
            assert_ne!(code, 0, "{name}: {out}{err}");
            assert!(runner.launches.borrow().is_empty(), "{name}");
            assert!(!out.contains("lockfile current"), "{name}: {out}");
            assert!(
                format!("{out}{err}").contains("cannot check maven"),
                "{name}: {out}{err}"
            );
        }
    }

    #[test]
    fn consumer_maven_check_that_writes_is_restored_and_failed() {
        let harness = maven_harness("maven-guard");
        let before = maven_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        runner.writes.borrow_mut().push((
            harness.workspace.join("third_party/maven_install.json"),
            b"rewritten\n".to_vec(),
        ));
        let (code, out, err) = run_with(&["update", "maven"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("check_wrote: third_party/maven_install.json (restored)"),
            "{err}"
        );
        assert_eq!(maven_bytes(&harness), before);
    }

    #[test]
    fn consumer_maven_failing_pin_fails_only_its_own_set() {
        let harness = maven_harness("maven-fail");
        let key = ScriptRunner::key_for(&maven_pin_call("maven"));
        let runner = ScriptRunner::new(&[(key.as_str(), Some(1))]);
        let (code, out, err) = run_with(
            &["update", "--apply", "maven", "tool_maven"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            format!("{out}{err}").contains("failed to update maven"),
            "{out}{err}"
        );
        assert!(out.contains("updated tool_maven"), "{out}{err}");
        assert_eq!(runner.launches.borrow().len(), 2);
    }

    const NUGET_FILES: [&str; 6] = [
        "MODULE.bazel",
        "third_party/dotnet/paket.dependencies",
        "third_party/dotnet/paket.lock",
        "third_party/dotnet/deps/paket.main.bzl",
        "third_party/dotnet_tools/paket.lock",
        "third_party/dotnet_tools/deps/paket.tools.bzl",
    ];

    fn nuget_harness(name: &str) -> Harness {
        let harness = consumer_harness(
            name,
            &format!(
                "{UV_MODULE}paket = use_extension(\"//third_party/dotnet/deps:paket.main_extension.bzl\", \"main_extension\")\n"
            ),
            true,
        );
        for dir in ["dotnet", "dotnet_tools"] {
            harness.write_source(
                &format!("third_party/{dir}/paket.dependencies"),
                "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json\n",
            );
            harness.write_source(&format!("third_party/{dir}/paket.lock"), "NUGET\n");
        }
        harness.write_source("third_party/dotnet/deps/paket.main.bzl", "main\n");
        harness.write_source("third_party/dotnet_tools/deps/paket.tools.bzl", "tools\n");
        harness
    }

    fn nuget_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        NUGET_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    fn stage(runner: &ScriptRunner, files: &[(&str, &str)]) {
        runner.staged_outputs.borrow_mut().extend(
            files
                .iter()
                .map(|(name, text)| ((*name).to_owned(), text.as_bytes().to_vec())),
        );
    }

    /// The `--output-folder` the single recorded launch ran with.
    fn scratch_out(runner: &ScriptRunner) -> std::path::PathBuf {
        let launches = runner.launches.borrow();
        let argv = &launches[0].argv;
        let at = argv
            .iter()
            .position(|arg| arg == "--output-folder")
            .expect("folder");
        std::path::PathBuf::from(&argv[at + 1])
    }

    #[test]
    fn consumer_nuget_check_without_drift_is_current_and_cleans_scratch() {
        let harness = nuget_harness("nuget-current");
        let before = nuget_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(
            &runner,
            &[
                ("paket.main.bzl", "main\n"),
                ("paket.main_extension.bzl", "ext\n"),
            ],
        );
        let (code, out, err) = run_with(&["update", "nuget"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("nuget lockfile current"), "{out}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1);
        let argv = &launches[0].argv;
        #[allow(clippy::disallowed_methods)]
        let mut head = dx_process::startup_argv(&[]);
        head.extend(strings_of(&[
            "run",
            "--config=dx_build_dev",
            "@rules_dx//dotnet:paket2bazel",
            "--",
            "--dependencies-file",
        ]));
        assert_eq!(argv[..head.len()], head[..], "{argv:?}");
        assert_eq!(argv.len(), head.len() + 3);
        assert_eq!(argv[head.len() + 1], "--output-folder");
        assert_eq!(launches[0].cwd, harness.workspace);
        assert!(launches[0].env.is_empty());
        assert!(!launches[0].hermetic);
        let deps = std::path::Path::new(&argv[head.len()]);
        assert!(!deps.starts_with(&harness.workspace), "{deps:?}");
        assert!(!deps.exists(), "scratch must be removed: {deps:?}");
        assert_eq!(nuget_bytes(&harness), before);
    }

    fn strings_of(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn consumer_nuget_check_with_drift_fails_listing_files_and_writes_nothing() {
        let harness = nuget_harness("nuget-drift");
        let before = nuget_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(
            &runner,
            &[
                ("paket.main.bzl", "new\n"),
                ("paket.main_extension.bzl", "ext\n"),
            ],
        );
        let (code, out, err) = run_with(&["update", "nuget"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("stale generated locks: third_party/dotnet/deps/paket.main.bzl;"),
            "{err}"
        );
        assert!(!err.contains("paket.main_extension.bzl"), "{err}");
        assert!(!out.contains("lockfile current"), "{out}");
        assert_eq!(nuget_bytes(&harness), before);
    }

    #[test]
    fn consumer_nuget_apply_writes_only_differing_declared_files() {
        let harness = nuget_harness("nuget-apply");
        let before = nuget_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(
            &runner,
            &[
                ("paket.main.bzl", "new\n"),
                ("paket.main_extension.bzl", "ext\n"),
            ],
        );
        let (code, out, err) = run_with(&["update", "--apply", "nuget"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("updated nuget"), "{out}");
        let after = nuget_bytes(&harness);
        for (index, rel) in NUGET_FILES.iter().enumerate() {
            if *rel == "third_party/dotnet/deps/paket.main.bzl" {
                assert_eq!(after[index], b"new\n", "{rel}");
            } else {
                assert_eq!(after[index], before[index], "{rel}");
            }
        }
        assert!(
            !harness
                .workspace
                .join("third_party/dotnet/deps/paket.main_extension.bzl")
                .exists()
        );
        assert!(!scratch_out(&runner).exists());
    }

    const HEX_MODULE: &str = "elixir = use_extension(\"//elixir/toolchain:extensions.bzl\", \"elixir\")\nelixir.lock(name = \"deps\", lock = \"//third_party/elixir:mix.lock\")\n";

    fn hex_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, HEX_MODULE, false);
        harness.write_source("third_party/elixir/mix.exs", "defmodule App do\nend\n");
        harness.write_source("third_party/elixir/mix.lock", "%{}\n");
        harness
    }

    #[test]
    fn consumer_hex_check_reports_drift_and_apply_writes_only_mix_lock() {
        let harness = hex_harness("hex-apply");
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("mix.lock", "%{\"jason\" => 1}\n")]);
        let (code, out, err) = run_with(&["update", "deps"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert_eq!(
            read_bytes(&harness, "third_party/elixir/mix.lock"),
            b"%{}\n"
        );

        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("mix.lock", "%{\"jason\" => 1}\n")]);
        let (code, out, err) = run_with(&["update", "--apply", "deps"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            read_bytes(&harness, "third_party/elixir/mix.lock"),
            b"%{\"jason\" => 1}\n"
        );
        assert_eq!(
            read_bytes(&harness, "third_party/elixir/mix.exs"),
            b"defmodule App do\nend\n"
        );
        let launches = runner.launches.borrow();
        assert!(launches[0].argv.iter().any(|arg| arg == "deps.update"));
    }

    #[test]
    fn consumer_mixed_mix_and_rebar_run_still_checks_the_mix_set() {
        let module = format!(
            "{HEX_MODULE}erlang = use_extension(\"//erlang/toolchain:repos.bzl\", \"erlang\")\nerlang.rebar(name = \"erl\", lock = \"//third_party/erlang:rebar.lock\")\n"
        );
        let harness = consumer_harness("hex-mixed", &module, false);
        harness.write_source("third_party/elixir/mix.exs", "defmodule App do\nend\n");
        harness.write_source("third_party/elixir/mix.lock", "%{}\n");
        harness.write_source("third_party/erlang/rebar.config", "{deps, []}.\n");
        harness.write_source("third_party/erlang/rebar.lock", "[].\n");
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("mix.lock", "%{\"jason\" => 1}\n")]);
        let (code, out, err) = run_with(&["update"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert!(launches[0].argv.iter().any(|arg| arg == "deps.get"));
        assert_eq!(
            read_bytes(&harness, "third_party/erlang/rebar.lock"),
            b"[].\n"
        );
    }

    const OPAM_MODULE: &str = "ocaml = use_extension(\"//ocaml/toolchain:extensions.bzl\", \"ocaml\")\nocaml.lock(name = \"ocaml_app\", lock = \"//ocaml/app:ocaml_app.opam.locked\")\n";

    const OPAM_FILE: &str = "opam-version: \"2.0\"\ndepends: [\n  \"astring\" {>= \"0.8.5\"}\n  \"ocaml\" {>= \"5.5\"}\n]\n";

    fn opam_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, OPAM_MODULE, false);
        harness.write_source("ocaml/app/ocaml_app.opam", OPAM_FILE);
        harness.write_source(
            "ocaml/app/ocaml_app.opam.locked",
            "opam-version: \"2.0\"\ndepends: [\n  \"astring\" {= \"0.8.5\"}\n]\n",
        );
        harness
    }

    #[test]
    fn consumer_opam_edits_the_constraint_and_runs_upgrade_then_lock() {
        let harness = opam_harness("opam-edit");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "ocaml_app:astring", "0.9.0"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            read_bytes(&harness, "ocaml/app/ocaml_app.opam"),
            OPAM_FILE.replace("0.8.5", "0.9.0").as_bytes()
        );
        let launches = runner.launches.borrow();
        assert!(
            launches
                .iter()
                .any(|run| run.argv.iter().any(|a| a == "upgrade"))
        );
        assert!(
            launches
                .iter()
                .any(|run| run.argv.iter().any(|a| a == "lock"))
        );
    }

    #[test]
    fn consumer_opam_refuses_an_undeclared_package_and_writes_nothing() {
        let harness = opam_harness("opam-undeclared");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "ocaml_app:lwt", "5.7.0"],
            &runner,
            &harness,
        );
        assert_ne!(code, 0, "{out}{err}");
        assert!(runner.launches.borrow().is_empty());
        assert_eq!(
            read_bytes(&harness, "ocaml/app/ocaml_app.opam"),
            OPAM_FILE.as_bytes()
        );
    }

    #[test]
    fn consumer_hex_offline_needs_network_and_launches_nothing() {
        let harness = hex_harness("hex-offline");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--offline", "deps"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert!(runner.launches.borrow().is_empty());
    }

    #[test]
    fn consumer_nuget_undeclared_or_missing_output_fails_and_writes_nothing() {
        for (name, files, want) in [
            (
                "undeclared",
                vec![
                    ("paket.main.bzl", "new\n"),
                    ("paket.main_extension.bzl", "ext2\n"),
                    ("paket.extra.bzl", "x\n"),
                ],
                "not declared in locks: paket.extra.bzl",
            ),
            (
                "missing",
                vec![("paket.main_extension.bzl", "ext\n")],
                "did not generate declared locks: third_party/dotnet/deps/paket.main.bzl",
            ),
        ] {
            let harness = nuget_harness(&format!("nuget-{name}"));
            let before = nuget_bytes(&harness);
            let runner = ScriptRunner::new(&[]);
            stage(&runner, &files);
            let (code, out, err) = run_with(&["update", "--apply", "nuget"], &runner, &harness);
            assert_eq!(code, 1, "{name}: {out}{err}");
            assert!(err.contains(want), "{name}: {err}");
            assert_eq!(nuget_bytes(&harness), before, "{name}");
        }
    }

    #[test]
    fn consumer_nuget_failing_run_writes_nothing() {
        let harness = nuget_harness("nuget-fail");
        let before = nuget_bytes(&harness);
        let key = ScriptRunner::key_for(&strings_of(&["paket2bazel"]));
        let runner = ScriptRunner::new(&[(key.as_str(), Some(1))]);
        stage(
            &runner,
            &[
                ("paket.main.bzl", "new\n"),
                ("paket.main_extension.bzl", "ext\n"),
            ],
        );
        let (code, out, err) = run_with(&["update", "--apply", "nuget"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            format!("{out}{err}").contains("failed to update nuget"),
            "{out}{err}"
        );
        assert_eq!(nuget_bytes(&harness), before);
    }

    #[test]
    fn consumer_nuget_offline_is_required_and_frozen_apply_runs() {
        let harness = nuget_harness("nuget-offline");
        for argv in [
            vec!["update", "--offline", "nuget"],
            vec!["update", "--apply", "--offline", "nuget"],
            vec!["update", "--apply", "nuget:Newtonsoft.Json"],
        ] {
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{argv:?}: {out}{err}");
            assert!(runner.launches.borrow().is_empty(), "{argv:?}");
            assert!(
                !format!("{out}{err}").contains("lockfile current"),
                "{argv:?}"
            );
        }
        let runner = ScriptRunner::new(&[]);
        let (_, out, err) = run_with(&["update", "--offline", "nuget"], &runner, &harness);
        assert!(
            format!("{out}{err}").contains("offline_required"),
            "{out}{err}"
        );
        let runner = ScriptRunner::new(&[]);
        stage(
            &runner,
            &[
                ("paket.main.bzl", "main\n"),
                ("paket.main_extension.bzl", "ext\n"),
            ],
        );
        let (code, out, err) = run_with(
            &["update", "--apply", "--frozen", "nuget"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.launches.borrow().len(), 1);
    }

    #[test]
    fn consumer_nuget_layout_refusal_never_launches() {
        let harness = nuget_harness("nuget-layout");
        harness.write_source(
            "third_party/dotnet/paket.dependencies",
            "source ../local-feed\n",
        );
        for mode in ["--output=text", "--apply"] {
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&["update", mode, "nuget"], &runner, &harness);
            assert_ne!(code, 0, "{out}{err}");
            assert!(runner.launches.borrow().is_empty());
            assert!(format!("{out}{err}").contains("not http(s)"), "{out}{err}");
        }
    }

    const GO_FILES: [&str; 3] = [
        "MODULE.bazel",
        "third_party/go/go.mod",
        "third_party/go/go.sum",
    ];

    const GO_MOD: &str =
        "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.7.0\n";

    fn go_harness(name: &str) -> Harness {
        let harness = consumer_harness(
            name,
            &format!(
                "{UV_MODULE}bazel_dep(name = \"rules_go\", version = \"0.50.1\")\ngo_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n"
            ),
            true,
        );
        harness.write_source("third_party/go/go.mod", GO_MOD);
        harness.write_source("third_party/go/go.sum", "old\n");
        harness
    }

    fn go_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        GO_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    /// The `-C` directory the single recorded launch ran in.
    fn go_scratch_out(runner: &ScriptRunner) -> std::path::PathBuf {
        let launches = runner.launches.borrow();
        let argv = &launches[0].argv;
        let at = argv.iter().position(|arg| arg == "-C").expect("-C");
        std::path::PathBuf::from(&argv[at + 1])
    }

    #[test]
    fn consumer_go_check_runs_go_in_scratch_with_forced_env_and_cleans_up() {
        let harness = go_harness("go-current");
        let before = go_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("go.sum", "old\n"), ("go.mod", GO_MOD)]);
        let (code, out, err) = run_with(&["update", "go"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("go lockfile current"), "{out}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1);
        let argv = &launches[0].argv;
        #[allow(clippy::disallowed_methods)]
        let mut head = dx_process::startup_argv(&[]);
        head.extend(strings_of(&[
            "run",
            "--config=dx_build_dev",
            "@rules_go//go",
            "--",
            "-C",
        ]));
        assert_eq!(argv[..head.len()], head[..], "{argv:?}");
        assert_eq!(
            argv[head.len() + 1..],
            strings_of(&["mod", "download", "all"])[..],
            "{argv:?}"
        );
        let out_dir = std::path::PathBuf::from(&argv[head.len()]);
        let scratch = out_dir.parent().expect("scratch").to_path_buf();
        assert!(out_dir.ends_with("out"));
        assert!(!scratch.starts_with(&harness.workspace));
        assert_eq!(launches[0].cwd, harness.workspace);
        assert!(!launches[0].hermetic);
        let env: std::collections::BTreeMap<&str, &str> = launches[0]
            .env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(env["GOENV"], "off");
        assert_eq!(env["GOWORK"], "off");
        assert_eq!(env["GOTOOLCHAIN"], "local");
        assert_eq!(env["GOFLAGS"], "-modcacherw");
        for key in ["GOMODCACHE", "GOPATH", "GOCACHE"] {
            let path = std::path::Path::new(env[key]);
            assert!(
                path.starts_with(&scratch) && !path.starts_with(&out_dir),
                "{key}"
            );
        }
        assert!(!scratch.exists(), "scratch must be removed: {scratch:?}");
        assert_eq!(go_bytes(&harness), before);
    }

    #[test]
    fn consumer_go_check_with_drift_fails_and_writes_nothing() {
        let harness = go_harness("go-drift");
        let before = go_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("go.sum", "new\n"), ("go.mod", GO_MOD)]);
        let (code, out, err) = run_with(&["update", "go"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("stale generated locks: third_party/go/go.sum;"),
            "{err}"
        );
        assert!(!out.contains("lockfile current"), "{out}");
        assert_eq!(go_bytes(&harness), before);
    }

    #[test]
    fn consumer_go_apply_writes_only_go_sum() {
        let harness = go_harness("go-apply");
        let before = go_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("go.sum", "new\n"), ("go.mod", GO_MOD)]);
        let (code, out, err) = run_with(&["update", "--apply", "go"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("updated go"), "{out}");
        let after = go_bytes(&harness);
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1], before[1]);
        assert_eq!(after[2], b"new\n");
        assert!(!go_scratch_out(&runner).exists());
    }

    #[test]
    fn consumer_go_changed_go_mod_fails_and_writes_nothing() {
        {
            let (name, files, want) = (
                "mod",
                vec![("go.sum", "new\n"), ("go.mod", "module changed\n")],
                "go changed go.mod",
            );
            let harness = go_harness(&format!("go-{name}"));
            let before = go_bytes(&harness);
            let runner = ScriptRunner::new(&[]);
            stage(&runner, &files);
            let (code, out, err) = run_with(&["update", "--apply", "go"], &runner, &harness);
            assert_eq!(code, 1, "{name}: {out}{err}");
            assert!(err.contains(want), "{name}: {err}");
            assert_eq!(go_bytes(&harness), before, "{name}");
        }
    }

    #[test]
    fn consumer_go_failing_run_fails_only_its_set() {
        let harness = go_harness("go-fail");
        let before = go_bytes(&harness);
        let key = ScriptRunner::key_for(&strings_of(&["@rules_go//go"]));
        let runner = ScriptRunner::new(&[(key.as_str(), Some(1))]);
        stage(&runner, &[("go.sum", "new\n"), ("go.mod", GO_MOD)]);
        let (code, out, err) = run_with(&["update", "--apply", "go"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            format!("{out}{err}").contains("failed to update go"),
            "{out}{err}"
        );
        assert_eq!(go_bytes(&harness), before);
    }

    #[test]
    fn consumer_go_offline_and_frozen_apply_never_launch() {
        let harness = go_harness("go-offline");
        for (argv, want) in [
            (vec!["update", "--offline", "go"], "offline_required"),
            (
                vec!["update", "--apply", "--offline", "go"],
                "offline_required",
            ),
            (vec!["update", "--apply", "--frozen", "go"], "frozen_locked"),
            (
                vec!["update", "--apply", "go:github.com/google/go-cmp"],
                "whole module graph",
            ),
        ] {
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{argv:?}: {out}{err}");
            assert!(runner.launches.borrow().is_empty(), "{argv:?}");
            let text = format!("{out}{err}");
            assert!(text.contains(want), "{argv:?}: {text}");
            assert!(!text.contains("lockfile current"), "{argv:?}");
        }
        // A frozen check only compares, so it still runs.
        let runner = ScriptRunner::new(&[]);
        stage(&runner, &[("go.sum", "old\n"), ("go.mod", GO_MOD)]);
        let (code, out, err) = run_with(&["update", "--frozen", "go"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(runner.launches.borrow().len(), 1);
    }

    #[test]
    fn consumer_go_layout_refusal_never_launches() {
        let harness = go_harness("go-layout");
        harness.write_source(
            "third_party/go/go.mod",
            &format!("{GO_MOD}\nreplace example.com/x => ../x\n"),
        );
        for mode in ["--output=text", "--apply"] {
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&["update", mode, "go"], &runner, &harness);
            assert_ne!(code, 0, "{out}{err}");
            assert!(runner.launches.borrow().is_empty());
            assert!(format!("{out}{err}").contains("local path"), "{out}{err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn scratch_removal_survives_a_read_only_module_cache() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = crate::exec::staged::Scratch::create("readonly").expect("scratch");
        let dir = scratch.0.join("gomodcache/pkg/mod");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("f.go"), "package f\n").expect("file");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
        let path = scratch.0.clone();
        drop(scratch);
        assert!(!path.exists(), "{path:?}");
    }

    #[cfg(unix)]
    #[test]
    fn consumer_check_with_a_lock_linking_outside_fails_before_launch() {
        for (argv, lock) in [
            (["update", "crates"], "consumer-rs/crates/Cargo.lock"),
            (["update", "web"], "consumer-npm/web/pnpm-lock.yaml"),
        ] {
            let harness = cargo_harness("link-out");
            let outside = harness
                .workspace
                .parent()
                .expect("parent")
                .join("outside-lock.txt");
            std::fs::write(&outside, b"outside\n").expect("outside");
            let at = harness.workspace.join(lock);
            std::fs::remove_file(&at).expect("remove lock");
            std::os::unix::fs::symlink(&outside, &at).expect("link");
            let runner = ScriptRunner::new(&[]);
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_eq!(code, 1, "{out}{err}");
            assert!(err.contains("leaves the workspace"), "{err}");
            assert!(runner.launches.borrow().is_empty());
            assert_eq!(std::fs::read(&outside).expect("outside"), b"outside\n");
        }
    }

    #[cfg(unix)]
    #[test]
    fn consumer_check_that_writes_through_an_inside_link_is_restored() {
        let harness = cargo_harness("link-in");
        let at = harness.workspace.join("consumer-rs/crates/Cargo.lock");
        let real = harness.workspace.join("consumer-rs/crates/real.lock");
        std::fs::rename(&at, &real).expect("rename");
        std::os::unix::fs::symlink(&real, &at).expect("link");
        let before = std::fs::read(&real).expect("real");
        let runner = ScriptRunner::new(&[]);
        runner
            .writes
            .borrow_mut()
            .push((at.clone(), b"rewritten\n".to_vec()));
        let (code, out, err) = run_with(&["update", "crates"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("check_wrote: consumer-rs/crates/Cargo.lock (restored)"),
            "{err}"
        );
        assert!(
            std::fs::symlink_metadata(&at)
                .expect("meta")
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&real).expect("real"), before);
    }

    #[test]
    fn consumer_cargo_wrong_manifest_name_exits_two() {
        let harness = consumer_harness(
            "cargo-manifest",
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//rs:Cargo.lock\", manifests = [\"//rs:cargo.toml\"])\n",
            true,
        );
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("must be named Cargo.toml"), "{err}");
        assert!(runner.calls.borrow().is_empty());
    }

    const SWIFT_MODULE: &str = "swift_deps = use_extension(\"@rules_swift_package_manager//:extensions.bzl\", \"swift_deps\")\nswift_deps.from_package(resolved = \"//apps/swift:Package.resolved\", swift = \"//apps/swift:Package.swift\")\n";
    const SWIFT_MANIFEST: &str = "apps/swift/Package.swift";

    fn swift_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, SWIFT_MODULE, true);
        harness.write_source(
            SWIFT_MANIFEST,
            "import PackageDescription\nlet package = Package(name: \"app\", dependencies: [\n    .package(url: \"https://github.com/apple/swift-argument-parser\", exact: \"1.8.2\"),\n    .package(url: \"https://github.com/apple/swift-log.git\", branch: \"main\"),\n], targets: [])\n",
        );
        harness.write_source(
            "apps/swift/Package.resolved",
            "{\"pins\":[],\"version\":3}\n",
        );
        harness
    }

    #[test]
    fn consumer_swiftpm_apply_edits_the_manifest_then_updates_the_pin() {
        let harness = swift_harness("swift-apply");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &[
                "update",
                "--apply",
                "swiftpm:swift-argument-parser",
                "1.9.0",
            ],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        let manifest = String::from_utf8(read_bytes(&harness, SWIFT_MANIFEST)).expect("utf8");
        assert!(manifest.contains("exact: \"1.9.0\""), "{manifest}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1, "{launches:?}");
        let argv = &launches[0].argv;
        assert_eq!(
            argv[argv.len() - 4..],
            strings_of(&["--", "package", "update", "swift-argument-parser"])[..],
            "{argv:?}"
        );
    }

    #[test]
    fn consumer_swiftpm_refusals_launch_nothing_and_keep_the_manifest() {
        let harness = swift_harness("swift-refuse");
        let before = read_bytes(&harness, SWIFT_MANIFEST);
        let runner = ScriptRunner::new(&[]);
        for package in ["swift-log", "missing"] {
            let (code, _, err) = run_with(
                &["update", "--apply", &format!("swiftpm:{package}"), "1.0.0"],
                &runner,
                &harness,
            );
            assert_ne!(code, 0, "{package}: {err}");
        }
        let (code, _, err) = run_with(
            &[
                "update",
                "--apply",
                "--frozen",
                "swiftpm:swift-argument-parser",
                "1.9.0",
            ],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        assert_eq!(read_bytes(&harness, SWIFT_MANIFEST), before);
        assert!(runner.launches.borrow().is_empty());
    }

    const CPAN_MODULE: &str = "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"cpan_deps\", lock = \"//perl/third_party:cpanfile.snapshot.lock.json\")\n";
    const CPAN_FILES: [&str; 3] = [
        "perl/third_party/cpanfile",
        "perl/third_party/cpanfile.snapshot",
        "perl/third_party/cpanfile.snapshot.lock.json",
    ];

    fn cpan_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, CPAN_MODULE, true);
        harness.write_source(
            "perl/third_party/cpanfile",
            "requires 'Text::CSV', '== 2.06';\n",
        );
        harness.write_source(
            "perl/third_party/cpanfile.snapshot",
            "# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  Text-CSV-2.06\n    pathname: I/IS/ISHIGAKI/Text-CSV-2.06.tar.gz\n",
        );
        harness.write_source(
            "perl/third_party/cpanfile.snapshot.lock.json",
            "{\"Text-CSV\":{\"strip_prefix\":\"Text-CSV-2.06\"}}\n",
        );
        harness.write_source(
            "perl/third_party/BUILD.bazel",
            "cpan_compiler(name = \"compiler\", cpanfile = \"cpanfile\", lockfile = \"cpanfile.snapshot.lock.json\")\n",
        );
        harness
    }

    fn cpan_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        CPAN_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    fn carton_tail(argv: &[String]) -> &[String] {
        let at = argv
            .iter()
            .position(|arg| arg == "@rules_dx//perl/tools:carton")
            .expect("carton target");
        &argv[at..]
    }

    #[test]
    fn consumer_cpan_check_installs_the_snapshot_in_deployment_mode() {
        let harness = cpan_harness("cpan-check");
        let before = cpan_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "cpan_deps"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1, "{launches:?}");
        let argv = &launches[0].argv;
        assert!(
            argv.iter()
                .any(|arg| arg.starts_with("--run_under=cd '") && arg.contains("perl/third_party")),
            "{argv:?}"
        );
        assert_eq!(
            carton_tail(argv),
            &strings_of(&[
                "@rules_dx//perl/tools:carton",
                "--",
                "install",
                "--deployment"
            ])[..],
            "{argv:?}"
        );
        assert_eq!(launches[0].env[0].0, "PERL_CARTON_PATH");
        assert_eq!(cpan_bytes(&harness), before);
    }

    #[test]
    fn consumer_cpan_apply_module_runs_carton_then_the_compiler() {
        let harness = cpan_harness("cpan-apply");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "cpan_deps:Text::CSV"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 2, "{launches:?}");
        assert_eq!(
            carton_tail(&launches[0].argv),
            &strings_of(&["@rules_dx//perl/tools:carton", "--", "update", "Text::CSV"])[..]
        );
        assert_eq!(
            launches[1].argv[launches[1].argv.len() - 3..],
            strings_of(&["//perl/third_party:compiler", "--", "--incremental"])[..]
        );
    }

    #[test]
    fn consumer_cpan_refusals_launch_nothing() {
        let harness = cpan_harness("cpan-refuse");
        let before = cpan_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--offline", "cpan_deps"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--frozen", "cpan_deps:Text::CSV"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--offline", "cpan_deps:Text::CSV"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = run_with(&["update", "cpan_deps:Text::CSV"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_set_unsupported"), "{err}");
        assert!(runner.launches.borrow().is_empty());
        assert_eq!(cpan_bytes(&harness), before);
    }

    const CABAL_MODULE: &str = "haskell = use_extension(\"//haskell/toolchain:extensions.bzl\", \"haskell\")\nhaskell.lock(name = \"hackage\", lock = \"//hs:haskell_deps.json\")\n";
    const CABAL_FILES: [&str; 4] = [
        "hs/cabal.project",
        "hs/cabal.project.freeze",
        "hs/haskell_deps.json",
        "hs/app.cabal",
    ];

    fn cabal_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, CABAL_MODULE, true);
        harness.write_source("hs/cabal.project", "packages: .\n");
        harness.write_source(
            "hs/cabal.project.freeze",
            "constraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n",
        );
        harness.write_source(
            "hs/haskell_deps.json",
            "{\"packages\":[{\"name\":\"base\",\"version\":\"4.20.2.0\"},{\"name\":\"safe\",\"version\":\"0.3.21\"}]}\n",
        );
        harness.write_source(
            "hs/app.cabal",
            "cabal-version: 3.0\nname: app\nversion: 0.1.0.0\nbuild-type: Simple\n\nlibrary\n  build-depends: base, safe ==0.3.21\n",
        );
        harness.write_source(
            "hs/BUILD.bazel",
            "haskell_deps_resolve(name = \"resolve\")\nhaskell_deps_resolve_test(name = \"resolve_test\")\n",
        );
        harness
    }

    fn cabal_bytes(harness: &Harness) -> Vec<Vec<u8>> {
        CABAL_FILES
            .iter()
            .map(|rel| read_bytes(harness, rel))
            .collect()
    }

    #[test]
    fn consumer_cabal_check_runs_the_resolve_test() {
        let harness = cabal_harness("cabal-check");
        let before = cabal_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "hackage"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1, "{launches:?}");
        assert_eq!(
            launches[0].argv.last().map(String::as_str),
            Some("//hs:resolve_test")
        );
        assert_eq!(cabal_bytes(&harness), before);
    }

    #[test]
    fn consumer_cabal_apply_runs_the_resolve_target() {
        let harness = cabal_harness("cabal-apply");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(&["update", "--apply", "hackage"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1, "{launches:?}");
        assert_eq!(
            launches[0].argv.last().map(String::as_str),
            Some("//hs:resolve")
        );
    }

    #[test]
    fn consumer_cabal_apply_package_rewrites_the_bound_then_resolves() {
        let harness = cabal_harness("cabal-bound");
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = run_with(
            &["update", "--apply", "hackage:safe", "0.3.22"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        let cabal = String::from_utf8(read_bytes(&harness, "hs/app.cabal")).expect("utf8");
        assert!(cabal.contains("safe ==0.3.22"), "{cabal}");
        let launches = runner.launches.borrow();
        assert_eq!(launches.len(), 1, "{launches:?}");
        assert_eq!(
            launches[0].argv.last().map(String::as_str),
            Some("//hs:resolve")
        );
    }

    #[test]
    fn consumer_cabal_refusals_launch_nothing() {
        let harness = cabal_harness("cabal-refuse");
        let before = cabal_bytes(&harness);
        let runner = ScriptRunner::new(&[]);
        let (code, _, err) = run_with(&["update", "--offline", "hackage"], &runner, &harness);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "--frozen", "hackage"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = run_with(
            &["update", "--apply", "hackage:missing", "1.0"],
            &runner,
            &harness,
        );
        assert_ne!(code, 0, "{err}");
        assert!(runner.launches.borrow().is_empty());
        assert_eq!(cabal_bytes(&harness), before);
    }
}
