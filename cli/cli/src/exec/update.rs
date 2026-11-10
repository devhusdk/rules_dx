use super::common::*;
use super::{update_add_missing, update_outdated, update_prune, update_unused};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, error_event, notice_event,
    with_correlation,
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
        if invocation.prune
            && let Err(code) = prune_unused(
                invocation,
                env.workspace,
                env.query_runner,
                &registry,
                env.out,
                env.err,
            )
        {
            return code;
        }
        let fetch = update_outdated::registry_fetch();
        if invocation.add_missing
            && let Err(code) = add_missing(
                invocation,
                env.workspace,
                env.query_runner,
                &registry,
                fetch.as_ref(),
                env.out,
                env.err,
            )
        {
            return code;
        }
        if invocation.applies()
            && !invocation.offline
            && !invocation.frozen
            && let Err(code) = add_named(
                invocation,
                env.workspace,
                &registry,
                fetch.as_ref(),
                env.out,
                env.err,
            )
        {
            return code;
        }
        return update_consumer::execute_configured(invocation, env, &registry, verbose, mode);
    }
    let message = no_sets_message(env.workspace);
    pre_exec(invocation.output, env.out, env.err, &message)
}

/// Why update has nothing to do: a manifest generate has not locked yet, or no sets at all.
pub(super) fn no_sets_message(workspace: &std::path::Path) -> String {
    let unlocked = dx_adopt::unlocked_manifests(workspace);
    if unlocked.is_empty() {
        "no dependency sets".to_owned()
    } else {
        format!(
            "no lock yet for {}; run dx generate --apply",
            unlocked.join(", ")
        )
    }
}

fn prune_unused(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    registry: &dx_adopt::dependency_sets::Registry,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<(), i32> {
    let resolved = match dx_adopt::dependency_sets::resolve(registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return Err(pre_exec(invocation.output, out, err, &error.to_string())),
    };
    let unused = update_unused::find(
        workspace,
        &resolved,
        query_runner,
        &invocation.bazel_startup_options,
    );
    match update_prune::prune(workspace, &unused) {
        Ok(removed) => {
            for item in removed {
                let _ = writeln!(
                    err,
                    "dx: update_pruned: {}: {} (removed from {}:{})",
                    item.set, item.name, item.manifest, item.line
                );
            }
            Ok(())
        }
        Err(message) => Err(operational(
            invocation,
            out,
            err,
            CODE_UPDATE_FAILED,
            &message,
        )),
    }
}

fn add_missing(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    registry: &dx_adopt::dependency_sets::Registry,
    fetch: &dyn dx_update::discover::Fetch,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<(), i32> {
    let resolved = match dx_adopt::dependency_sets::resolve(registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return Err(pre_exec(invocation.output, out, err, &error.to_string())),
    };
    report_added(
        invocation,
        update_add_missing::add_missing(
            workspace,
            &resolved,
            fetch,
            query_runner,
            &invocation.bazel_startup_options,
        ),
        out,
        err,
    )
}

fn add_named(
    invocation: &Invocation,
    workspace: &std::path::Path,
    registry: &dx_adopt::dependency_sets::Registry,
    fetch: &dyn dx_update::discover::Fetch,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<(), i32> {
    let resolved = match dx_adopt::dependency_sets::resolve(registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return Err(pre_exec(invocation.output, out, err, &error.to_string())),
    };
    report_added(
        invocation,
        update_add_missing::add_named(workspace, &resolved, fetch),
        out,
        err,
    )
}

fn report_added(
    invocation: &Invocation,
    added: Result<Vec<update_add_missing::Added>, String>,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> Result<(), i32> {
    match added {
        Ok(added) => {
            for item in added {
                let _ = writeln!(
                    err,
                    "dx: update_added: {}: {} {} (added to {})",
                    item.set, item.name, item.version, item.manifest
                );
            }
            Ok(())
        }
        Err(message) => Err(operational(
            invocation,
            out,
            err,
            CODE_UPDATE_FAILED,
            &message,
        )),
    }
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
            RunMode::Check => "dx update",
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
        "cannot check {name}: no check tool is set up for it; refresh with `dx update --apply {name}`"
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
        | dx_update::backend::BackendError::NonUtf8Path { .. }
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
    /// Who held the slots when a wait limit ran out, kept for the caller to report.
    busy: Option<String>,
}

impl SlotGate<'_> {
    /// Takes the slot before a Bazel run; `Err` carries the stop signal, or 1 with `busy` set.
    fn before(&mut self, argv: &[String]) -> Result<(), i32> {
        let taken = self.phase.need(
            dx_update::backend::runs_bazel(argv),
            &mut *self.err,
            &mut *self.events,
        );
        match taken {
            Ok(_) => Ok(()),
            Err(dx_plan::slots::Stop::Signal(signo)) => Err(signo),
            Err(dx_plan::slots::Stop::Busy(holder)) => {
                self.busy = Some(holder);
                Err(1)
            }
        }
    }
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
                if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
                    return exit;
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
                if verbose && let Some(detail) = details.get(outcome.set.as_str()) {
                    let message = &detail.message;
                    if let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
                        return exit;
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
                if verbose
                    && let Err(exit) = check_stdout_write(writeln!(
                        out,
                        "blocked {} (depends on a failed update)",
                        outcome.set
                    ))
                {
                    return exit;
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
    if verbose
        && let Err(exit) = check_stdout_write(writeln!(out, "{}", status_summary(report, mode)))
    {
        return exit;
    }
    exit
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

#[cfg(test)]
#[path = "update_check.rs"]
mod update_check;
#[path = "update_consumer.rs"]
mod update_consumer;
pub(super) use update_consumer::windows_env;

/// The pinned uv and interpreter a configured uv set refreshes with.
pub(super) fn configured_uv_closure(
    runner: &dyn dx_process::Runner,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
    workspace: &std::path::Path,
    startup: &[String],
    offline: bool,
) -> dx_update::backend::UvClosure {
    update_consumer::uv_closure(runner, query_runner, workspace, startup, offline)
}
#[cfg(test)]
#[path = "update_live.rs"]
mod update_live;

#[cfg(test)]
mod tests {
    use super::*;
    use dx_adopt::dependency_sets::{DependencySet, Ecosystem, Registry};
    use dx_error::DxError;
    use dx_plan::args::parse;
    use std::path::Path;

    struct NoQuery;

    impl dx_plan::resolve::QueryRunner for NoQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &Path,
        ) -> std::io::Result<dx_plan::resolve::QueryResult> {
            Err(std::io::Error::other("no bazel"))
        }
    }

    fn workspace(manifest: &str, source: &str) -> (tempfile::TempDir, Registry) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("web")).unwrap();
        std::fs::write(dir.path().join("web/package.json"), manifest).unwrap();
        std::fs::write(dir.path().join("web/index.js"), source).unwrap();
        std::fs::write(
            dir.path().join("web/pnpm-lock.yaml"),
            "lockfileVersion: '9.0'\n\npackages:\n\n  left-pad@1.3.0:\n    resolution: {integrity: sha512-x}\n\n  react@18.0.0:\n    resolution: {integrity: sha512-y}\n",
        )
        .unwrap();
        let registry = Registry {
            sets: vec![DependencySet {
                name: "npm".to_owned(),
                ecosystem: Ecosystem::Npm,
                manifests: vec!["web/package.json".to_owned()],
                locks: vec!["web/pnpm-lock.yaml".to_owned()],
                scopes: Vec::new(),
                repository: None,
            }],
        };
        (dir, registry)
    }

    fn json_invocation(flag: &str) -> Invocation {
        parse(&[
            "update".to_owned(),
            "--apply".to_owned(),
            flag.to_owned(),
            "--output=json".to_owned(),
        ])
        .expect("parse")
    }

    fn assert_runtime_failure(code: i32, out: &[u8], err: &[u8]) {
        let out = String::from_utf8_lossy(out);
        assert_eq!(code, dx_process::operational_code(), "{out}");
        assert!(out.contains("\"code\":\"update_failed\""), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert!(!String::from_utf8_lossy(err).contains("usage"));
    }

    #[test]
    fn a_failed_add_missing_is_a_runtime_error_on_stdout() {
        let (dir, registry) = workspace(
            "{\n  \"dependencies\": {\n    \"react\": \"18.0.0\"\n  }\n}\n",
            "import pad from 'left-pad';\nimport React from 'react';\n",
        );
        let fetch =
            |url: &str| -> Result<String, DxError> { Err(DxError::failure(format!("404 {url}"))) };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = add_missing(
            &json_invocation("--add-missing"),
            dir.path(),
            &NoQuery,
            &registry,
            &fetch,
            &mut out,
            &mut err,
        )
        .expect_err("fails");
        assert_runtime_failure(code, &out, &err);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_prune_is_a_runtime_error_on_stdout() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, registry) = workspace(
            "{\n  \"dependencies\": {\n    \"left-pad\": \"1.3.0\",\n    \"react\": \"18.0.0\"\n  }\n}\n",
            "import React from 'react';\n",
        );
        let manifest = dir.path().join("web/package.json");
        std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new()
            .write(true)
            .open(&manifest)
            .is_ok()
        {
            return;
        }
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = prune_unused(
            &json_invocation("--prune"),
            dir.path(),
            &NoQuery,
            &registry,
            &mut out,
            &mut err,
        )
        .expect_err("fails");
        assert_runtime_failure(code, &out, &err);
    }
}
