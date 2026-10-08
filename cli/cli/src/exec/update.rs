use super::common::*;
use crate::args::{Command, Invocation};
use crate::reports::plan_reports;
use dx_output::{
    change_event, command_finished, command_started, error_event, mutation_event, notice_event,
    with_correlation, write_event, ChangeEvent, ChangeKind, Edit, FinishedCounts, MutationOutcome,
    NoticeEvent, OutputMode,
};
use std::collections::BTreeMap;

pub(crate) fn execute_update(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Update,
        "update dispatch guards commands"
    );
    match plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(env.err, &error.to_string()),
    }
    let verbose = invocation.chatty();
    if invocation.check {
        execute_update_check(invocation, env, verbose)
    } else {
        execute_update_default(invocation, env, verbose)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Update,
    Check,
}

impl RunMode {
    fn command_text(self) -> &'static str {
        match self {
            RunMode::Update => "dx update",
            RunMode::Check => "dx update --check",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            RunMode::Update => "update",
            RunMode::Check => "check",
        }
    }
}

fn execute_update_check(invocation: &Invocation, env: Env<'_>, verbose: bool) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let resolved = match dx_update::selector::resolve(&invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let summary = offline_summary(
        display_summary(&resolved, RunMode::Check),
        invocation.offline,
    );
    if invocation.dry_run {
        return emit_check_dry_run(invocation, out, workspace, &resolved, &summary, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "check") {
            let _ = write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    let (attempted, details) =
        run_update_check_backends(&resolved, runner, workspace, invocation.offline);
    finish_selected_update(
        invocation,
        out,
        err,
        &resolved,
        attempted,
        details,
        verbose,
        RunMode::Check,
    )
}

fn execute_update_default(invocation: &Invocation, env: Env<'_>, verbose: bool) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let resolved = match dx_update::selector::resolve(&invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let summary = offline_summary(
        display_summary(&resolved, RunMode::Update),
        invocation.offline,
    );
    if invocation.dry_run {
        return emit_update_dry_run(invocation, out, workspace, &resolved, &summary, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    let (attempted, details) =
        run_update_backends(&resolved, runner, workspace, invocation.offline);
    finish_selected_update(
        invocation,
        out,
        err,
        &resolved,
        attempted,
        details,
        verbose,
        RunMode::Update,
    )
}

fn finish_selected_update(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    attempted: Vec<dx_update::outcome::SetOutcome>,
    details: BTreeMap<dx_update::sets::SetId, String>,
    verbose: bool,
    mode: RunMode,
) -> i32 {
    let selected: Vec<String> = resolved.keys().map(|set| set.name().to_owned()).collect();
    let depends: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let report = match dx_update::outcome::aggregate(&selected, &attempted, &depends) {
        Ok(report) => report,
        Err(error) => {
            return operational(invocation, out, err, CODE_UPDATE_FAILED, &error.to_string());
        }
    };
    let exit = dx_update::report::exit_code(&report);
    if invocation.output == OutputMode::Json {
        emit_update_json(out, err, &report, &details, verbose, exit, mode)
    } else {
        emit_update_text(out, err, &report, &details, verbose, exit, mode)
    }
}

fn emit_update_dry_run(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    summary: &str,
    verbose: bool,
) -> i32 {
    emit_dry_run(
        invocation,
        out,
        summary,
        verbose,
        "default",
        resolved
            .iter()
            .map(|(set, request)| {
                describe_update_plan(workspace, *set, request, invocation.offline)
            })
            .collect(),
    )
}

fn emit_check_dry_run(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    summary: &str,
    verbose: bool,
) -> i32 {
    emit_dry_run(
        invocation,
        out,
        summary,
        verbose,
        "check",
        resolved
            .iter()
            .map(|(set, request)| describe_check_plan(workspace, *set, request, invocation.offline))
            .collect(),
    )
}

fn emit_dry_run(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    summary: &str,
    verbose: bool,
    mode: &str,
    plans: Vec<String>,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), true, mode) {
            let _ = write_event(out, &event);
        }
        let finished = command_finished(0, &FinishedCounts::default());
        let _ = write_event(out, &finished);
    } else if verbose {
        let _ = writeln!(out, "{summary}");
        for plan in &plans {
            let _ = writeln!(out, "{plan}");
        }
    }
    0
}

fn describe_update_plan(
    workspace: &std::path::Path,
    set: dx_update::sets::SetId,
    request: &dx_update::selector::SetRequest,
    offline: bool,
) -> String {
    match dx_update::backend::plan(workspace, set, request, offline) {
        Ok(dx_update::backend::BackendPlan::Run { argv, .. }) => {
            format!("Would update {}: {}", set.name(), argv.join(" "))
        }
        Ok(dx_update::backend::BackendPlan::Noop) => {
            format!(
                "Would leave {} pinned (manual pins; nothing to resolve)",
                set.name()
            )
        }
        Err(error) => format!("Cannot update {}: {error}", set.name()),
    }
}

fn describe_check_plan(
    workspace: &std::path::Path,
    set: dx_update::sets::SetId,
    request: &dx_update::selector::SetRequest,
    offline: bool,
) -> String {
    match dx_update::backend::check(workspace, set, request, offline) {
        Ok(dx_update::backend::CheckPlan::Run { argv, .. }) => {
            format!("Would check {}: {}", set.name(), argv.join(" "))
        }
        Ok(dx_update::backend::CheckPlan::Pinned) => {
            format!(
                "Would leave {} pinned (manual pins; nothing to resolve)",
                set.name()
            )
        }
        Ok(dx_update::backend::CheckPlan::Unavailable) => unavailable_message(set),
        Err(error) => format!("Cannot check {}: {error}", set.name()),
    }
}

fn record(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<dx_update::sets::SetId, String>,
    set: dx_update::sets::SetId,
    status: dx_update::outcome::SetStatus,
    message: String,
) {
    attempted.push(dx_update::outcome::SetOutcome {
        set: set.name().to_owned(),
        status,
    });
    details.insert(set, message);
}

fn unavailable_message(set: dx_update::sets::SetId) -> String {
    format!(
        "cannot check {}: no qualified check backend; refresh with `dx update {}`",
        set.name(),
        set.name()
    )
}

fn run_update_backends(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    offline: bool,
) -> (
    Vec<dx_update::outcome::SetOutcome>,
    BTreeMap<dx_update::sets::SetId, String>,
) {
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<dx_update::sets::SetId, String> = BTreeMap::new();
    for (set, request) in resolved {
        match dx_update::backend::plan(workspace, *set, request, offline) {
            Err(error) => {
                let message = format!("cannot update {}: {error}", set.name());
                match error {
                    dx_update::backend::BackendError::Unsupported { .. } => record(
                        &mut attempted,
                        &mut details,
                        *set,
                        dx_update::outcome::SetStatus::Unsupported,
                        message,
                    ),
                    dx_update::backend::BackendError::OfflineRequired { .. } => record(
                        &mut attempted,
                        &mut details,
                        *set,
                        dx_update::outcome::SetStatus::Failed,
                        format!("failed to update {}: {error}", set.name()),
                    ),
                }
                continue;
            }
            Ok(dx_update::backend::BackendPlan::Noop) => {
                record(
                    &mut attempted,
                    &mut details,
                    *set,
                    dx_update::outcome::SetStatus::Pinned,
                    pinned_line(*set),
                );
            }
            Ok(dx_update::backend::BackendPlan::Run { argv, env: extra }) => {
                run_update_backend(BackendRun {
                    attempted: &mut attempted,
                    details: &mut details,
                    set: *set,
                    request,
                    runner,
                    workspace,
                    argv: &argv,
                    extra: &extra,
                    mode: RunMode::Update,
                });
            }
        }
    }
    (attempted, details)
}

fn run_update_check_backends(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    offline: bool,
) -> (
    Vec<dx_update::outcome::SetOutcome>,
    BTreeMap<dx_update::sets::SetId, String>,
) {
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<dx_update::sets::SetId, String> = BTreeMap::new();
    for (set, request) in resolved {
        match dx_update::backend::check(workspace, *set, request, offline) {
            Err(error) => {
                let message = format!("cannot check {}: {error}", set.name());
                match error {
                    dx_update::backend::BackendError::Unsupported { .. } => record(
                        &mut attempted,
                        &mut details,
                        *set,
                        dx_update::outcome::SetStatus::Unsupported,
                        message,
                    ),
                    dx_update::backend::BackendError::OfflineRequired { .. } => record(
                        &mut attempted,
                        &mut details,
                        *set,
                        dx_update::outcome::SetStatus::Failed,
                        format!("failed to check {}: {error}", set.name()),
                    ),
                }
                continue;
            }
            Ok(dx_update::backend::CheckPlan::Pinned) => {
                record(
                    &mut attempted,
                    &mut details,
                    *set,
                    dx_update::outcome::SetStatus::Pinned,
                    pinned_line(*set),
                );
            }
            Ok(dx_update::backend::CheckPlan::Unavailable) => {
                record(
                    &mut attempted,
                    &mut details,
                    *set,
                    dx_update::outcome::SetStatus::Unsupported,
                    unavailable_message(*set),
                );
            }
            Ok(dx_update::backend::CheckPlan::Run { argv, env: extra }) => {
                run_update_backend(BackendRun {
                    attempted: &mut attempted,
                    details: &mut details,
                    set: *set,
                    request,
                    runner,
                    workspace,
                    argv: &argv,
                    extra: &extra,
                    mode: RunMode::Check,
                });
            }
        }
    }
    (attempted, details)
}

struct BackendRun<'a> {
    attempted: &'a mut Vec<dx_update::outcome::SetOutcome>,
    details: &'a mut BTreeMap<dx_update::sets::SetId, String>,
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
    let verb = mode.verb();
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    match runner.run(argv, workspace, &env_refs) {
        Err(error) => {
            record(
                attempted,
                details,
                set,
                dx_update::outcome::SetStatus::Failed,
                format!(
                    "failed to {verb} {}: failed to launch updater: {error}",
                    set.name()
                ),
            );
        }
        Ok(status) => match status.code {
            Some(0) => {
                let (status, message) = match mode {
                    RunMode::Update => (
                        dx_update::outcome::SetStatus::Updated,
                        success_line(set, request),
                    ),
                    RunMode::Check => (dx_update::outcome::SetStatus::Current, current_line(set)),
                };
                record(attempted, details, set, status, message);
            }
            Some(code) => {
                record(
                    attempted,
                    details,
                    set,
                    dx_update::outcome::SetStatus::Failed,
                    format!("failed to {verb} {}: updater exited {code}", set.name()),
                );
            }
            None => {
                record(
                    attempted,
                    details,
                    set,
                    dx_update::outcome::SetStatus::Failed,
                    format!(
                        "failed to {verb} {}: updater terminated by signal",
                        set.name()
                    ),
                );
            }
        },
    }
}

fn emit_update_json(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<dx_update::sets::SetId, String>,
    verbose: bool,
    exit: i32,
    _mode: RunMode,
) -> i32 {
    for outcome in &report.outcomes {
        let set_name = outcome.set.as_str();
        let correlation = format!("update:{set_name}");
        match outcome.status {
            dx_update::outcome::ReportedStatus::Updated => {
                let message = details
                    .get(&parse_set(set_name))
                    .cloned()
                    .unwrap_or_else(|| format!("updated {set_name}"));
                if let Ok(event) = notice_event(&NoticeEvent {
                    level: "info".to_owned(),
                    code: "update_set_success".to_owned(),
                    message,
                    related_command: Some("update".to_owned()),
                    scope: Some(vec![set_name.to_owned()]),
                    path: None,
                    language: None,
                    import: None,
                }) {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    if let Some(manifest) = live_success_manifest(set_name) {
                        if let Ok(file_events) = project_manifest_events(&manifest) {
                            for file_event in &file_events {
                                let _ = write_event(out, file_event);
                            }
                        }
                    }
                    let _ = write_event(out, &event);
                }
            }
            dx_update::outcome::ReportedStatus::Current
            | dx_update::outcome::ReportedStatus::Pinned => {
                let (code, fallback) = match outcome.status {
                    dx_update::outcome::ReportedStatus::Current => {
                        ("update_set_current", format!("{set_name} lockfile current"))
                    }
                    _ => (
                        "update_set_pinned",
                        format!("{set_name} pins are manual; nothing to resolve"),
                    ),
                };
                let message = details
                    .get(&parse_set(set_name))
                    .cloned()
                    .unwrap_or(fallback);
                if let Ok(event) = notice_event(&NoticeEvent {
                    level: "info".to_owned(),
                    code: code.to_owned(),
                    message,
                    related_command: Some("update".to_owned()),
                    scope: Some(vec![set_name.to_owned()]),
                    path: None,
                    language: None,
                    import: None,
                }) {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    let _ = write_event(out, &event);
                }
            }
            dx_update::outcome::ReportedStatus::Unsupported => {
                let message = details
                    .get(&parse_set(set_name))
                    .cloned()
                    .unwrap_or_else(|| format!("cannot update {set_name}"));
                if let Ok(event) = error_event(
                    "update_set_unsupported",
                    &message,
                    None,
                    None,
                    Some("execute"),
                ) {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    let _ = write_event(out, &event);
                }
                let _ = writeln!(err, "dx: update_set_unsupported: {message}");
            }
            dx_update::outcome::ReportedStatus::Failed => {
                let message = details
                    .get(&parse_set(set_name))
                    .cloned()
                    .unwrap_or_else(|| format!("failed to update {set_name}"));
                let code = if message.contains(CODE_OFFLINE_REQUIRED) {
                    CODE_OFFLINE_REQUIRED
                } else {
                    CODE_UPDATE_FAILED
                };
                if let Ok(event) = error_event(code, &message, None, None, Some("execute")) {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    let _ = write_event(out, &event);
                }
                let _ = writeln!(err, "dx: {code}: {message}");
            }
            dx_update::outcome::ReportedStatus::Blocked => {
                let message = format!("blocked {set_name} (depends on a failed update)");
                if let Ok(event) = notice_event(&NoticeEvent {
                    level: "warning".to_owned(),
                    code: "update_set_blocked".to_owned(),
                    message: message.clone(),
                    related_command: Some("update".to_owned()),
                    scope: Some(vec![set_name.to_owned()]),
                    path: None,
                    language: None,
                    import: None,
                }) {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    let _ = write_event(out, &event);
                }
                if verbose {
                    let _ = writeln!(out, "{message}");
                }
            }
        }
    }
    if let Some(plan) = dx_update::recovery::plan(report) {
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "warning".to_owned(),
            code: dx_update::recovery::RECOVERY_CODE.to_owned(),
            message: plan.message.clone(),
            related_command: Some("update".to_owned()),
            scope: Some(plan.retry_sets.clone()),
            path: None,
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
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
    let _ = write_event(out, &finished);
    exit
}

fn emit_update_text(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<dx_update::sets::SetId, String>,
    verbose: bool,
    exit: i32,
    mode: RunMode,
) -> i32 {
    for outcome in &report.outcomes {
        match outcome.status {
            dx_update::outcome::ReportedStatus::Updated
            | dx_update::outcome::ReportedStatus::Current
            | dx_update::outcome::ReportedStatus::Pinned => {
                if verbose {
                    if let Some(message) = details.get(&parse_set(outcome.set.as_str())) {
                        let _ = writeln!(out, "{message}");
                    }
                }
            }
            dx_update::outcome::ReportedStatus::Unsupported => {
                if let Some(message) = details.get(&parse_set(outcome.set.as_str())) {
                    let _ = writeln!(err, "dx: update_set_unsupported: {message}");
                }
            }
            dx_update::outcome::ReportedStatus::Failed => {
                if let Some(message) = details.get(&parse_set(outcome.set.as_str())) {
                    let code = if message.contains(CODE_OFFLINE_REQUIRED) {
                        CODE_OFFLINE_REQUIRED
                    } else {
                        CODE_UPDATE_FAILED
                    };
                    let _ = writeln!(err, "dx: {code}: {message}");
                }
            }
            dx_update::outcome::ReportedStatus::Blocked => {
                if verbose {
                    let _ = writeln!(out, "blocked {} (depends on a failed update)", outcome.set);
                }
            }
        }
    }
    if let Some(plan) = dx_update::recovery::plan(report) {
        let _ = writeln!(
            err,
            "dx: {}: {}",
            dx_update::recovery::RECOVERY_CODE,
            plan.message
        );
    }
    if verbose {
        let _ = writeln!(out, "{}", status_summary(report, mode));
    }
    exit
}

fn live_success_manifest(set_name: &str) -> Option<dx_update::manifest::CommittedManifest> {
    if set_name == "go" {
        Some(dx_update::manifest::CommittedManifest {
            set: "go".to_owned(),
            changes: Vec::new(),
        })
    } else {
        None
    }
}

pub(crate) fn project_manifest_events(
    manifest: &dx_update::manifest::CommittedManifest,
) -> Result<Vec<serde_json::Value>, dx_update::manifest::ManifestError> {
    let projected = dx_update::manifest::project(manifest)?;
    let correlation = format!("update:{}", manifest.set);
    let mut events = Vec::with_capacity(projected.len() * 2);
    for file in &projected {
        let kind = match file.kind {
            dx_update::manifest::CommittedKind::Modify => ChangeKind::Modify,
            dx_update::manifest::CommittedKind::Create => ChangeKind::Create,
        };
        let change = ChangeEvent {
            path: file.path.clone(),
            kind,
            source_digest: file.source_digest.clone(),
            edits: vec![Edit {
                start: file.start_byte,
                end: file.end_byte,
                replacement: file.replacement.clone(),
            }],
        };
        let change =
            change_event(&change).map_err(|_| dx_update::manifest::ManifestError::BadContent {
                path: file.path.clone(),
            })?;
        let change = with_correlation(change, &correlation).map_err(|_| {
            dx_update::manifest::ManifestError::BadContent {
                path: file.path.clone(),
            }
        })?;
        events.push(change);
        let mutation =
            mutation_event(&file.path, kind, MutationOutcome::Applied, None).map_err(|_| {
                dx_update::manifest::ManifestError::BadContent {
                    path: file.path.clone(),
                }
            })?;
        let mutation = with_correlation(mutation, &correlation).map_err(|_| {
            dx_update::manifest::ManifestError::BadContent {
                path: file.path.clone(),
            }
        })?;
        events.push(mutation);
    }
    Ok(events)
}

fn parse_set(name: &str) -> dx_update::sets::SetId {
    dx_update::sets::SetId::parse(name).unwrap_or(dx_update::sets::SetId::Cargo)
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
        dx_update::sets::SetId::Go => "go pins are manual (track Gazelle for the shared go_deps extension; widen via `dx bump gomod:<module> <version>`); nothing to resolve"
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
#[cfg(test)]
#[path = "update_live.rs"]
mod update_live;
