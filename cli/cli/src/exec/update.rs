use super::common::*;
use dx_output::{
    command_finished, command_started, error_event, notice_event, with_correlation, FinishedCounts,
    NoticeEvent, OutputMode,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::plan_reports;
use std::collections::BTreeMap;

pub(crate) fn execute_update(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Update,
        "update dispatch guards commands"
    );
    match plan_reports(
        env.workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(invocation.output, env.out, env.err, &error.to_string()),
    }
    let verbose = invocation.chatty();
    let mode = RunMode::of(invocation);
    if let Some(registry) = match dx_adopt::dependency_sets::load(env.workspace) {
        Ok(registry) => registry,
        Err(error) => return pre_exec(invocation.output, env.out, env.err, &error.to_string()),
    } {
        return update_consumer::execute_configured(invocation, env, &registry, verbose, mode);
    }
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let resolved = match dx_update::selector::resolve(&invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let summary = frozen_summary(
        offline_summary(display_summary(&resolved, mode), invocation.offline),
        invocation.frozen,
    );
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(
            out,
            command_started(invocation.command.name(), mode.mode_text()),
        ) {
            return exit;
        }
    } else if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    let mut gate = SlotGate {
        err: &mut *err,
        events: &mut *out,
        phase: dx_plan::slots::Phase::new(invocation.command),
    };
    let outcomes = run_backends(&resolved, runner, workspace, invocation, mode, &mut gate);
    drop(gate);
    let (attempted, details) = match outcomes {
        Ok(outcomes) => outcomes,
        Err(signo) => {
            return super::failure::cancelled(err, signo);
        }
    };
    let selected: Vec<String> = resolved.keys().map(|set| set.name().to_owned()).collect();
    let locks: BTreeMap<String, Vec<String>> = resolved
        .keys()
        .map(|set| {
            (
                set.name().to_owned(),
                set.locks().iter().map(|path| (*path).to_owned()).collect(),
            )
        })
        .collect();
    finish_update(FinishUpdate {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Update,
    Check,
}

impl RunMode {
    fn of(invocation: &Invocation) -> RunMode {
        if invocation.applies() {
            RunMode::Update
        } else {
            RunMode::Check
        }
    }

    fn mode_text(self) -> &'static str {
        match self {
            RunMode::Update => "default",
            RunMode::Check => "check",
        }
    }

    fn command_text(self) -> &'static str {
        match self {
            RunMode::Update => "dx update",
            RunMode::Check => "dx update --check",
        }
    }

    fn retry_mode(self) -> dx_update::recovery::RetryMode {
        match self {
            RunMode::Update => dx_update::recovery::RetryMode::Apply,
            RunMode::Check => dx_update::recovery::RetryMode::Check,
        }
    }

    fn verb(self) -> &'static str {
        match self {
            RunMode::Update => "update",
            RunMode::Check => "check",
        }
    }
}

struct FinishUpdate<'a> {
    invocation: &'a Invocation,
    out: &'a mut dyn std::io::Write,
    err: &'a mut dyn std::io::Write,
    selected: Vec<String>,
    attempted: Vec<dx_update::outcome::SetOutcome>,
    details: BTreeMap<String, SetDetail>,
    verbose: bool,
    mode: RunMode,
    locks: BTreeMap<String, Vec<String>>,
}

fn finish_update(finish: FinishUpdate<'_>) -> i32 {
    let FinishUpdate {
        invocation,
        out,
        err,
        selected,
        attempted,
        details,
        verbose,
        mode,
        locks,
    } = finish;
    let depends: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let report = match dx_update::outcome::aggregate(&selected, &attempted, &depends) {
        Ok(report) => report,
        Err(error) => {
            return operational(invocation, out, err, CODE_UPDATE_FAILED, &error.to_string());
        }
    };
    let recovery = dx_update::recovery::plan(&report, mode.retry_mode(), &locks);
    if invocation.output == OutputMode::Json {
        emit_update_json(out, err, &report, &details, verbose, recovery.as_ref())
    } else {
        emit_update_text(
            out,
            err,
            &report,
            &details,
            verbose,
            recovery.as_ref(),
            mode,
        )
    }
}

pub(super) fn record_named(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<String, SetDetail>,
    name: &str,
    status: dx_update::outcome::SetStatus,
    message: String,
) {
    attempted.push(dx_update::outcome::SetOutcome {
        set: name.to_owned(),
        status,
    });
    details.insert(
        name.to_owned(),
        SetDetail {
            message,
            failure_code: None,
        },
    );
}

/// What the emitters need to know about one set beyond its status.
pub(super) struct SetDetail {
    message: String,
    /// Typed code for a `Failed` set; `None` falls back to `update_failed`.
    failure_code: Option<&'static str>,
}

/// Record a backend error, keeping its typed code for failed sets.
pub(super) fn record_backend_error(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<String, SetDetail>,
    error: &dx_update::backend::BackendError,
    verb: &str,
    name: &str,
) {
    let (status, message) = backend_error_status(error, verb, name);
    let failure_code = (status == dx_update::outcome::SetStatus::Failed).then(|| error.code());
    record_named(attempted, details, name, status, message);
    if let Some(detail) = details.get_mut(name) {
        detail.failure_code = failure_code;
    }
}

fn unavailable_message(name: &str) -> String {
    format!(
        "cannot check {name}: no qualified check backend; refresh with `dx update --apply {name}`"
    )
}

type Status = (dx_update::outcome::SetStatus, String);

fn backend_error_status(
    error: &dx_update::backend::BackendError,
    verb: &str,
    name: &str,
) -> Status {
    match error {
        dx_update::backend::BackendError::Unsupported { .. } => (
            dx_update::outcome::SetStatus::Unsupported,
            format!("cannot {verb} {name}: {error}"),
        ),
        dx_update::backend::BackendError::OfflineRequired { .. }
        | dx_update::backend::BackendError::FrozenLocked { .. }
        | dx_update::backend::BackendError::BackendUnavailable { .. } => (
            dx_update::outcome::SetStatus::Failed,
            format!("failed to {verb} {name}: {error}"),
        ),
    }
}

fn runner_status(
    launched: std::io::Result<dx_process::ChildStatus>,
    verb: &str,
    name: &str,
    ok: Status,
) -> Status {
    match launched {
        Err(error) => (
            dx_update::outcome::SetStatus::Failed,
            format!("failed to {verb} {name}: failed to launch updater: {error}"),
        ),
        Ok(status) => match status.code {
            Some(0) => ok,
            Some(code) => (
                dx_update::outcome::SetStatus::Failed,
                format!("failed to {verb} {name}: updater exited {code}"),
            ),
            None => (
                dx_update::outcome::SetStatus::Failed,
                format!("failed to {verb} {name}: updater terminated by signal"),
            ),
        },
    }
}

struct SlotGate<'a> {
    err: &'a mut dyn std::io::Write,
    events: &'a mut dyn std::io::Write,
    phase: dx_plan::slots::Phase,
}

impl SlotGate<'_> {
    fn before(&mut self, argv: &[String]) -> Result<(), i32> {
        self.phase
            .need(
                dx_update::backend::runs_bazel(argv),
                &mut *self.err,
                &mut *self.events,
            )
            .map(|_| ())
    }
}

type Outcomes = (
    Vec<dx_update::outcome::SetOutcome>,
    BTreeMap<String, SetDetail>,
);

fn run_backends(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    invocation: &Invocation,
    mode: RunMode,
    gate: &mut SlotGate<'_>,
) -> Result<Outcomes, i32> {
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<String, SetDetail> = BTreeMap::new();
    let head = invocation.bazel_head();
    let verb = mode.verb();
    for (set, request) in resolved {
        let name = set.name();
        let planned = match mode {
            RunMode::Update => dx_update::backend::plan(
                workspace,
                *set,
                request,
                invocation.offline,
                invocation.frozen,
                &head,
            )
            .map(|plan| match plan {
                dx_update::backend::BackendPlan::Run { argv, env } => {
                    dx_update::backend::CheckPlan::Run { argv, env }
                }
                dx_update::backend::BackendPlan::Noop => dx_update::backend::CheckPlan::Pinned,
            }),
            RunMode::Check => dx_update::backend::check(
                workspace,
                *set,
                request,
                invocation.offline,
                invocation.frozen,
                &head,
            ),
        };
        match planned {
            Err(error) => {
                record_backend_error(&mut attempted, &mut details, &error, verb, name);
            }
            Ok(dx_update::backend::CheckPlan::Pinned) => {
                record_named(
                    &mut attempted,
                    &mut details,
                    name,
                    dx_update::outcome::SetStatus::Pinned,
                    pinned_line(*set),
                );
            }
            Ok(dx_update::backend::CheckPlan::Unavailable) => {
                record_named(
                    &mut attempted,
                    &mut details,
                    name,
                    dx_update::outcome::SetStatus::Unsupported,
                    unavailable_message(name),
                );
            }
            Ok(dx_update::backend::CheckPlan::Run { argv, env: extra }) => {
                gate.before(&argv)?;
                let held = gate.phase.slot().map(|slot| slot.runner(runner));
                run_update_backend(BackendRun {
                    attempted: &mut attempted,
                    details: &mut details,
                    set: *set,
                    request,
                    runner: held
                        .as_ref()
                        .map_or(runner, |held| held as &dyn dx_process::Runner),
                    workspace,
                    argv: &argv,
                    extra: &extra,
                    mode,
                });
            }
        }
    }
    Ok((attempted, details))
}

struct BackendRun<'a> {
    attempted: &'a mut Vec<dx_update::outcome::SetOutcome>,
    details: &'a mut BTreeMap<String, SetDetail>,
    set: dx_update::sets::SetId,
    request: &'a dx_update::selector::SetRequest,
    runner: &'a dyn dx_process::Runner,
    workspace: &'a std::path::Path,
    argv: &'a [String],
    extra: &'a [(String, String)],
    mode: RunMode,
}

fn run_update_backend(run: BackendRun<'_>) {
    let BackendRun {
        attempted,
        details,
        set,
        request,
        runner,
        workspace,
        argv,
        extra,
        mode,
    } = run;
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let ok = match mode {
        RunMode::Update => (
            dx_update::outcome::SetStatus::Updated,
            success_line(set, request),
        ),
        RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
    };
    let (status, message) = runner_status(
        runner.run(argv, workspace, &env_refs),
        mode.verb(),
        set.name(),
        ok,
    );
    record_named(attempted, details, set.name(), status, message);
}

fn emit_update_json(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<String, SetDetail>,
    verbose: bool,
    recovery: Option<&dx_update::recovery::RecoveryPlan>,
) -> i32 {
    let exit = dx_update::report::exit_code(report);
    for outcome in &report.outcomes {
        let set_name = outcome.set.as_str();
        let correlation = format!("update:{set_name}");
        match outcome.status {
            dx_update::outcome::ReportedStatus::Updated => {
                let message = details
                    .get(set_name)
                    .map(|detail| detail.message.clone())
                    .unwrap_or_else(|| format!("updated {set_name}"));
                let built = notice_event(&NoticeEvent {
                    related_command: Some("update".to_owned()),
                    scope: Some(vec![set_name.to_owned()]),
                    ..NoticeEvent::info(CODE_UPDATE_SET_SUCCESS, message)
                })
                .map(|event| with_correlation(event.clone(), &correlation).unwrap_or(event));
                if let Err(exit) = emit_built(out, built) {
                    return exit;
                }
            }
            dx_update::outcome::ReportedStatus::Current
            | dx_update::outcome::ReportedStatus::Pinned => {
                let (code, fallback) = match outcome.status {
                    dx_update::outcome::ReportedStatus::Current => (
                        CODE_UPDATE_SET_CURRENT,
                        format!("{set_name} lockfile current"),
                    ),
                    _ => (
                        CODE_UPDATE_SET_PINNED,
                        format!("{set_name} pins are manual; nothing to resolve"),
                    ),
                };
                let message = details
                    .get(set_name)
                    .map(|detail| detail.message.clone())
                    .unwrap_or(fallback);
                if let Err(exit) = emit_built(
                    out,
                    notice_event(&NoticeEvent {
                        related_command: Some("update".to_owned()),
                        scope: Some(vec![set_name.to_owned()]),
                        ..NoticeEvent::info(code, message)
                    })
                    .map(|event| with_correlation(event.clone(), &correlation).unwrap_or(event)),
                ) {
                    return exit;
                }
            }
            dx_update::outcome::ReportedStatus::Unsupported => {
                let message = details
                    .get(set_name)
                    .map(|detail| detail.message.clone())
                    .unwrap_or_else(|| format!("cannot update {set_name}"));
                if let Err(exit) = emit_built(
                    out,
                    error_event(
                        "update_set_unsupported",
                        &message,
                        None,
                        None,
                        Some("execute"),
                    )
                    .map(|event| with_correlation(event.clone(), &correlation).unwrap_or(event)),
                ) {
                    return exit;
                }
                let _ = writeln!(err, "dx: update_set_unsupported: {message}");
            }
            dx_update::outcome::ReportedStatus::Failed => {
                let message = details
                    .get(set_name)
                    .map(|detail| detail.message.clone())
                    .unwrap_or_else(|| format!("failed to update {set_name}"));
                let code = details
                    .get(set_name)
                    .and_then(|detail| detail.failure_code)
                    .unwrap_or(CODE_UPDATE_FAILED);
                if let Err(exit) = emit_built(
                    out,
                    error_event(code, &message, None, None, Some("execute")).map(|event| {
                        with_correlation(event.clone(), &correlation).unwrap_or(event)
                    }),
                ) {
                    return exit;
                }
                let _ = writeln!(err, "dx: {code}: {message}");
            }
            dx_update::outcome::ReportedStatus::Blocked => {
                let message = format!("blocked {set_name} (depends on a failed update)");
                if let Err(exit) = emit_built(
                    out,
                    notice_event(&NoticeEvent {
                        related_command: Some("update".to_owned()),
                        scope: Some(vec![set_name.to_owned()]),
                        ..NoticeEvent::warning(CODE_UPDATE_SET_BLOCKED, message.clone())
                    })
                    .map(|event| with_correlation(event.clone(), &correlation).unwrap_or(event)),
                ) {
                    return exit;
                }
                if verbose {
                    if let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
                        return exit;
                    }
                }
            }
        }
    }
    if let Some(plan) = recovery {
        if let Err(exit) = emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("update".to_owned()),
                scope: Some(plan.retry_sets.clone()),
                ..NoticeEvent::warning(dx_update::recovery::RECOVERY_CODE, plan.message.clone())
            }),
        ) {
            return exit;
        }
        let _ = writeln!(
            err,
            "dx: {}: {}",
            dx_update::recovery::RECOVERY_CODE,
            plan.message
        );
    }
    let finished = command_finished(
        exit,
        &FinishedCounts {
            results_complete: Some(true),
            ..FinishedCounts::default()
        },
    );
    if let Err(exit) = emit_event(out, &finished) {
        return exit;
    }
    exit
}

fn emit_update_text(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<String, SetDetail>,
    verbose: bool,
    recovery: Option<&dx_update::recovery::RecoveryPlan>,
    mode: RunMode,
) -> i32 {
    let exit = dx_update::report::exit_code(report);
    for outcome in &report.outcomes {
        match outcome.status {
            dx_update::outcome::ReportedStatus::Updated
            | dx_update::outcome::ReportedStatus::Current
            | dx_update::outcome::ReportedStatus::Pinned => {
                if verbose {
                    if let Some(detail) = details.get(outcome.set.as_str()) {
                        let message = &detail.message;
                        if let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
                            return exit;
                        }
                    }
                }
            }
            dx_update::outcome::ReportedStatus::Unsupported => {
                if let Some(detail) = details.get(outcome.set.as_str()) {
                    let message = &detail.message;
                    let _ = writeln!(err, "dx: update_set_unsupported: {message}");
                }
            }
            dx_update::outcome::ReportedStatus::Failed => {
                if let Some(detail) = details.get(outcome.set.as_str()) {
                    let code = detail.failure_code.unwrap_or(CODE_UPDATE_FAILED);
                    let message = &detail.message;
                    let _ = writeln!(err, "dx: {code}: {message}");
                }
            }
            dx_update::outcome::ReportedStatus::Blocked => {
                if verbose {
                    if let Err(exit) = check_stdout_write(writeln!(
                        out,
                        "blocked {} (depends on a failed update)",
                        outcome.set
                    )) {
                        return exit;
                    }
                }
            }
        }
    }
    if let Some(plan) = recovery {
        let _ = writeln!(
            err,
            "dx: {}: {}",
            dx_update::recovery::RECOVERY_CODE,
            plan.message
        );
    }
    if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", status_summary(report, mode))) {
            return exit;
        }
    }
    exit
}

fn display_summary(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    mode: RunMode,
) -> String {
    let command = match mode {
        RunMode::Update => "Running update for",
        RunMode::Check => "Running update --check for",
    };
    let all_full = resolved.len() == dx_update::sets::SetId::ALL.len()
        && resolved
            .values()
            .all(|request| *request == dx_update::selector::SetRequest::Full);
    if all_full {
        return format!("{command} all dependency sets");
    }
    let mut parts = Vec::new();
    for (set, request) in resolved {
        match request {
            dx_update::selector::SetRequest::Full => parts.push(set.name().to_owned()),
            dx_update::selector::SetRequest::Packages(packages) => {
                for package in packages {
                    parts.push(format!("{}:{package}", set.name()));
                }
            }
        }
    }
    format!("{command} {}", parts.join(", "))
}

fn status_summary(report: &dx_update::outcome::UpdateReport, mode: RunMode) -> String {
    let mut parts = Vec::new();
    for (status, verb) in [
        (dx_update::outcome::ReportedStatus::Updated, "updated"),
        (dx_update::outcome::ReportedStatus::Current, "current"),
        (dx_update::outcome::ReportedStatus::Pinned, "pinned"),
        (
            dx_update::outcome::ReportedStatus::Unsupported,
            "unsupported",
        ),
        (dx_update::outcome::ReportedStatus::Failed, "failed"),
        (dx_update::outcome::ReportedStatus::Blocked, "blocked"),
    ] {
        let count = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == status)
            .count();
        if count > 0 {
            parts.push(format!("{count} {verb}"));
        }
    }
    format!("{}: {}", mode.command_text(), parts.join(", "))
}

fn current_line(set: dx_update::sets::SetId) -> String {
    format!(
        "{} lockfile current ({})",
        set.name(),
        set.locks().join(", ")
    )
}

fn pinned_line(set: dx_update::sets::SetId) -> String {
    match set {
        dx_update::sets::SetId::Go => "go pins are manual (track Gazelle for the shared go_deps extension; widen via `dx bump go:<module> <version>`); nothing to resolve"
            .to_owned(),
        dx_update::sets::SetId::Ruby => {
            "ruby pins are manual (regenerate with `bundle lock` on the seed host); nothing to resolve"
                .to_owned()
        }
        dx_update::sets::SetId::PowerShell => "powershell pins are manual (hand-regenerate PSGallery.lock.json; builds never run Install-Module); nothing to resolve"
            .to_owned(),
        other => format!("{} pins are manual; nothing to resolve", other.name()),
    }
}

fn success_line(set: dx_update::sets::SetId, request: &dx_update::selector::SetRequest) -> String {
    match request {
        dx_update::selector::SetRequest::Full => match set {
            dx_update::sets::SetId::Cargo => {
                "updated cargo (rust/tests/fixtures/hello/Cargo.lock, cargo-bazel-lock.json)"
                    .to_owned()
            }
            dx_update::sets::SetId::Npm => "updated npm (pnpm-lock.yaml)".to_owned(),
            dx_update::sets::SetId::NpmTools => {
                "updated npm-tools (quality/tools/javascript/pnpm-lock.yaml)".to_owned()
            }
            dx_update::sets::SetId::NpmAdopt => {
                "updated npm-adopt (examples/adopt-js-ts/pnpm-lock.yaml)".to_owned()
            }
            dx_update::sets::SetId::NpmAdoptPolyglot => {
                "updated npm-adopt-polyglot (examples/adopt-polyglot/pnpm-lock.yaml)".to_owned()
            }
            dx_update::sets::SetId::Maven => {
                "updated maven (third_party/jvm/maven_install.json)".to_owned()
            }
            dx_update::sets::SetId::NuGet => {
                "updated nuget (third_party/dotnet/paket.lock)".to_owned()
            }
            dx_update::sets::SetId::Uv => {
                "updated uv (python/tests/fixtures/hello/uv.lock)".to_owned()
            }
            dx_update::sets::SetId::UvTools => {
                "updated uv-tools (quality/tools/python/uv.lock)".to_owned()
            }
            dx_update::sets::SetId::UvAdopt => {
                "updated uv-adopt (examples/adopt-python/uv.lock)".to_owned()
            }
            dx_update::sets::SetId::UvAdoptPolyglot => {
                "updated uv-adopt-polyglot (examples/adopt-polyglot/uv.lock)".to_owned()
            }
            dx_update::sets::SetId::Go
            | dx_update::sets::SetId::Ruby
            | dx_update::sets::SetId::PowerShell => {
                let locks = set.locks().join(", ");
                format!("updated {} ({locks})", set.name())
            }
        },
        dx_update::selector::SetRequest::Packages(packages) => {
            let locks = set.locks().join(", ");
            let selections: Vec<String> = packages
                .iter()
                .map(|package| format!("{}:{package}", set.name()))
                .collect();
            if locks.is_empty() {
                format!("updated {} (nothing to update)", selections.join(", "))
            } else {
                format!("updated {} ({locks})", selections.join(", "))
            }
        }
    }
}

#[cfg(test)]
#[path = "update_check.rs"]
mod update_check;
#[path = "update_consumer.rs"]
mod update_consumer;
#[cfg(test)]
#[path = "update_live.rs"]
mod update_live;
