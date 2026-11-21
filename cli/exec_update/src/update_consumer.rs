use super::{
    RunMode, SetDetail, SlotGate, record_backend_error, record_named, runner_status,
    unavailable_message,
};
use crate::staged::{StagedError, ToolRun, run_staged_core, write_outputs};
use dx_adopt::dependency_sets::{self, Ecosystem, ResolvedSet, ResolvedTarget, Selection};
use dx_exec_core::common::{
    Env, check_stdout_write, emit_built, frozen_summary, offline_summary, pre_exec, windows_env,
};
use dx_exec_reports::pinned_tool::QueryBazel;
use dx_exec_reports::uv_tools::UvTools;
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
            return dx_exec_core::failure::slot_stop(invocation.output, out, err, stop);
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
    if let Some((status, message)) = precheck(workspace, set, mode) {
        record_named(attempted, details, &set.name, status, message);
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
    let closure = closure_for(set.ecosystem, tools, rejected);
    let planned = plan_target(workspace, target, invocation, mode, &closure);
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
    let launched = launch_all(gate, runner, workspace, &runs)?;
    let ok = match mode {
        RunMode::Update => (
            dx_update::outcome::SetStatus::Updated,
            success_line(set, &target.request),
        ),
        RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
    };
    let (status, message) = runner_status(launched, verb, &set.name, ok);
    if let Some(wrote) = guarded.and_then(|guard| guard.restore(workspace)) {
        record_check_wrote(attempted, details, set, verb, &wrote);
        return Ok(());
    }
    record_named(attempted, details, &set.name, status, message);
    Ok(())
}

fn launch_all(
    gate: &mut SlotGate<'_>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    runs: &[dx_update::backend::Run],
) -> Result<std::io::Result<dx_process::ChildStatus>, dx_plan::slots::Stop> {
    let mut launched = Ok(dx_process::ChildStatus::exited(0));
    for run in runs {
        if let Err(code) = gate.before(&run.argv) {
            return Err(stop_of(gate, code));
        }
        launched = launch(gate, runner, workspace, &run.argv, &run.env);
        if !matches!(&launched, Ok(status) if status.code == Some(0)) {
            break;
        }
    }
    Ok(launched)
}

fn record_check_wrote(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<String, SetDetail>,
    set: &dependency_sets::ResolvedSet,
    verb: &str,
    wrote: &[String],
) {
    let hint = check_wrote_hint(set.ecosystem);
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
}

fn plan_target(
    workspace: &std::path::Path,
    target: &ResolvedTarget,
    invocation: &Invocation,
    mode: RunMode,
    closure: &dx_update::backend::UvClosure,
) -> Result<dx_update::backend::CheckPlan, dx_update::backend::BackendError> {
    let head = invocation.bazel_head();
    match mode {
        RunMode::Update => dx_update::backend::plan_configured(
            workspace,
            &target.set,
            &target.request,
            invocation.offline,
            invocation.frozen,
            &head,
            closure,
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
            &target.set,
            &target.request,
            invocation.offline,
            invocation.frozen,
            &head,
            closure,
        ),
    }
}

fn precheck(
    workspace: &std::path::Path,
    set: &dependency_sets::ResolvedSet,
    mode: RunMode,
) -> Option<(dx_update::outcome::SetStatus, String)> {
    let verb = mode.verb();
    if let Err(error) = dependency_sets::check_files(workspace, set, mode == RunMode::Check) {
        return Some((
            dx_update::outcome::SetStatus::Failed,
            format!("failed to {verb} {}: {error}", set.name),
        ));
    }
    if let Err(error) = dependency_sets::check_backend_settings(workspace, set) {
        return Some((
            settings_status(&error),
            format!("cannot {verb} {}: {error}", set.name),
        ));
    }
    if mode == RunMode::Check
        && let Err(error) = dependency_sets::check_lock_ready(workspace, set)
    {
        return Some((
            dx_update::outcome::SetStatus::Unsupported,
            format!("cannot {verb} {}: {error}", set.name),
        ));
    }
    None
}

fn settings_status(error: &dependency_sets::DependencySetsError) -> dx_update::outcome::SetStatus {
    match error {
        dependency_sets::DependencySetsError::UnsupportedSettings { .. }
        | dependency_sets::DependencySetsError::UnsupportedNpmLayout { .. }
        | dependency_sets::DependencySetsError::MissingPinnedLock { .. }
        | dependency_sets::DependencySetsError::UnsupportedMavenHub { .. }
        | dependency_sets::DependencySetsError::UnsupportedCargoHub { .. }
        | dependency_sets::DependencySetsError::UnsupportedGoLayout { .. } => {
            dx_update::outcome::SetStatus::Unsupported
        }
        dependency_sets::DependencySetsError::Read { .. }
        | dependency_sets::DependencySetsError::NotUtf8 { .. }
        | dependency_sets::DependencySetsError::InvalidToml { .. }
        | dependency_sets::DependencySetsError::Module { .. }
        | dependency_sets::DependencySetsError::InvalidName { .. }
        | dependency_sets::DependencySetsError::EscapingPath { .. }
        | dependency_sets::DependencySetsError::InvalidManifest { .. }
        | dependency_sets::DependencySetsError::DuplicateName { .. }
        | dependency_sets::DependencySetsError::ConflictingOwnership { .. }
        | dependency_sets::DependencySetsError::UnknownSelector { .. }
        | dependency_sets::DependencySetsError::InvalidPackage { .. }
        | dependency_sets::DependencySetsError::NoOwningSet { .. }
        | dependency_sets::DependencySetsError::MissingFile { .. }
        | dependency_sets::DependencySetsError::OutsideWorkspace { .. }
        | dependency_sets::DependencySetsError::StaleJuliaLock { .. }
        | dependency_sets::DependencySetsError::StaleCpanLock { .. }
        | dependency_sets::DependencySetsError::StaleCabalLock { .. }
        | dependency_sets::DependencySetsError::InvalidRepository { .. }
        | dependency_sets::DependencySetsError::Hubs { .. } => {
            dx_update::outcome::SetStatus::Failed
        }
    }
}

fn closure_for(
    ecosystem: Ecosystem,
    tools: &mut UvTools<'_>,
    rejected: bool,
) -> dx_update::backend::UvClosure {
    match ecosystem {
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
        | Ecosystem::Julia
        | Ecosystem::Hex
        | Ecosystem::Composer
        | Ecosystem::Dub
        | Ecosystem::Fpm
        | Ecosystem::Zig
        | Ecosystem::LuaRocks
        | Ecosystem::Opam
        | Ecosystem::SwiftPm
        | Ecosystem::Cabal
        | Ecosystem::Ansible
        | Ecosystem::Bicep => dx_update::backend::UvClosure::default(),
    }
}

fn check_wrote_hint(ecosystem: Ecosystem) -> &'static str {
    match ecosystem {
        Ecosystem::Cargo => {
            "; declare `lockfile` in crate.from_cargo and list it in the set's locks"
        }
        Ecosystem::Maven => {
            "; the maven.install lock_file must be listed in the set's locks and MODULE.bazel must not change"
        }
        Ecosystem::Uv
        | Ecosystem::Npm
        | Ecosystem::Bazel
        | Ecosystem::Ruby
        | Ecosystem::PowerShell
        | Ecosystem::Go
        | Ecosystem::Cpan
        | Ecosystem::Pub
        | Ecosystem::Julia
        | Ecosystem::Hex
        | Ecosystem::Composer
        | Ecosystem::Dub
        | Ecosystem::Nuget
        | Ecosystem::Fpm
        | Ecosystem::Zig
        | Ecosystem::LuaRocks
        | Ecosystem::Opam
        | Ecosystem::SwiftPm
        | Ecosystem::Cabal
        | Ecosystem::Ansible
        | Ecosystem::Bicep => "",
    }
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
                    // A path that does not resolve inside the workspace is never restored.
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
                // The check swapped in a link that leaves the workspace or points elsewhere.
                let link = workspace.join(&rel);
                if std::fs::symlink_metadata(&link)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                    && let Err(error) = std::fs::remove_file(&link)
                {
                    tracing::warn!(%error, path = %link.display(), "cannot remove the escaping link");
                }
                if let Some(bytes) = &before
                    && let Err(error) = dx_atomic_fs::write_atomic(&stored, bytes)
                {
                    tracing::warn!(%error, path = %stored.display(), "cannot restore the checked file");
                }
                wrote.push(rel);
                continue;
            }
            if std::fs::read(&stored).ok() == before {
                continue;
            }
            if let Err(error) = match &before {
                Some(bytes) => dx_atomic_fs::write_atomic(&stored, bytes),
                None => std::fs::remove_file(&stored),
            } {
                tracing::warn!(%error, path = %stored.display(), "cannot restore the checked file");
            }
            wrote.push(rel);
        }
        (!wrote.is_empty()).then_some(wrote)
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
#[path = "update_consumer_tests.rs"]
mod tests;
