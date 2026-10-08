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
    let checking = invocation.check || !invocation.apply;
    let mode = mode_of(checking);
    let resolved = match dx_update::selector::resolve(&invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(env.err, &error.to_string()),
    };
    let summary = offline_summary(check_summary(&resolved, checking), invocation.offline);
    if invocation.dry_run {
        return emit_update_dry_run(invocation, env.out, &summary, checking, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, mode) {
            let _ = write_event(env.out, &event);
        }
    } else if verbose {
        let _ = writeln!(env.out, "{summary}");
    }
    if checking {
        execute_update_check(invocation, env, &resolved, verbose)
    } else {
        execute_update_apply(invocation, env, &resolved, verbose)
    }
}

fn execute_update_check(
    invocation: &Invocation,
    env: Env<'_>,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    verbose: bool,
) -> i32 {
    let Env {
        runner,
        workspace,
        out,
        err,
        ..
    } = env;
    let (attempted, details) = run_check_backends(resolved, runner, workspace, invocation.offline);
    finish_update(
        invocation, out, err, resolved, &attempted, &details, verbose,
    )
}

fn execute_update_apply(
    invocation: &Invocation,
    env: Env<'_>,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    verbose: bool,
) -> i32 {
    let Env {
        runner,
        workspace,
        out,
        err,
        ..
    } = env;
    let (attempted, details) = run_update_backends(resolved, runner, workspace, invocation.offline);
    finish_update(
        invocation, out, err, resolved, &attempted, &details, verbose,
    )
}

fn finish_update(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    attempted: &[dx_update::outcome::SetOutcome],
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    verbose: bool,
) -> i32 {
    let selected: Vec<String> = resolved.keys().map(|set| set.name().to_owned()).collect();
    let depends: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let report = match dx_update::outcome::aggregate(&selected, attempted, &depends) {
        Ok(report) => report,
        Err(error) => {
            return operational(invocation, out, err, CODE_UPDATE_FAILED, &error.to_string());
        }
    };
    let exit = dx_update::report::exit_code(&report);
    if invocation.output == OutputMode::Json {
        emit_update_json(out, err, &report, details, verbose, exit)
    } else {
        emit_update_text(out, err, &report, details, verbose, exit)
    }
}

fn emit_update_dry_run(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    summary: &str,
    checking: bool,
    verbose: bool,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), true, mode_of(checking)) {
            let _ = write_event(out, &event);
        }
        let finished = command_finished(0, &FinishedCounts::default());
        let _ = write_event(out, &finished);
    } else if verbose {
        let _ = writeln!(out, "{summary}");
        if checking {
            let _ = writeln!(out, "Would check without writing anything");
        } else {
            let _ = writeln!(out, "Would run resolvers for the selected sets");
        }
    }
    0
}

fn mode_of(checking: bool) -> &'static str {
    if checking {
        "check"
    } else {
        "default"
    }
}

fn record_outcome(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<dx_update::sets::SetId, SetDetail>,
    set: dx_update::sets::SetId,
    status: dx_update::outcome::SetStatus,
    detail: SetDetail,
) {
    attempted.push(dx_update::outcome::SetOutcome {
        set: set.name().to_owned(),
        status,
    });
    details.insert(set, detail);
}

fn run_check_backends(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    offline: bool,
) -> (
    Vec<dx_update::outcome::SetOutcome>,
    BTreeMap<dx_update::sets::SetId, SetDetail>,
) {
    use dx_update::outcome::SetStatus;
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<dx_update::sets::SetId, SetDetail> = BTreeMap::new();
    for (set, request) in resolved {
        let plan = match dx_update::backend::check(workspace, *set, request, offline) {
            Ok(plan) => plan,
            Err(error) => {
                match error {
                    dx_update::backend::BackendError::Unsupported { reason, .. } => {
                        record_outcome(
                            &mut attempted,
                            &mut details,
                            *set,
                            SetStatus::Unsupported,
                            SetDetail::Failed {
                                message: format!("unsupported update: {reason}"),
                            },
                        );
                    }
                    dx_update::backend::BackendError::OfflineRequired { .. } => {
                        record_outcome(
                            &mut attempted,
                            &mut details,
                            *set,
                            SetStatus::Failed,
                            SetDetail::Failed {
                                message: format!("{error}"),
                            },
                        );
                    }
                }
                continue;
            }
        };
        match plan {
            dx_update::backend::CheckPlan::Unavailable { reason } => {
                record_outcome(
                    &mut attempted,
                    &mut details,
                    *set,
                    SetStatus::Unavailable,
                    SetDetail::Note { message: reason },
                );
            }
            dx_update::backend::CheckPlan::Run { argv, env: extra } => {
                run_check_backend(CheckRun {
                    attempted: &mut attempted,
                    details: &mut details,
                    set: *set,
                    runner,
                    workspace,
                    argv: &argv,
                    extra: &extra,
                });
            }
        }
    }
    (attempted, details)
}

struct CheckRun<'a> {
    attempted: &'a mut Vec<dx_update::outcome::SetOutcome>,
    details: &'a mut BTreeMap<dx_update::sets::SetId, SetDetail>,
    set: dx_update::sets::SetId,
    runner: &'a dyn dx_process::Runner,
    workspace: &'a std::path::Path,
    argv: &'a [String],
    extra: &'a [(String, String)],
}

fn run_check_backend(run: CheckRun<'_>) {
    use dx_update::outcome::SetStatus;
    let CheckRun {
        attempted,
        details,
        set,
        runner,
        workspace,
        argv,
        extra,
    } = run;
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    match runner.run(argv, workspace, &env_refs) {
        Err(error) => {
            record_outcome(
                attempted,
                details,
                set,
                SetStatus::Failed,
                SetDetail::Failed {
                    message: format!(
                        "failed to check {}: failed to launch checker: {error}",
                        set.name()
                    ),
                },
            );
        }
        Ok(status) => match status.code {
            Some(0) => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Current,
                    SetDetail::Note {
                        message: current_line(set),
                    },
                );
            }
            Some(_) => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Failed,
                    SetDetail::Failed {
                        message: stale_line(set),
                    },
                );
            }
            None => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Failed,
                    SetDetail::Failed {
                        message: format!(
                            "failed to check {}: checker terminated by signal",
                            set.name()
                        ),
                    },
                );
            }
        },
    }
}

fn run_update_backends(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    offline: bool,
) -> (
    Vec<dx_update::outcome::SetOutcome>,
    BTreeMap<dx_update::sets::SetId, SetDetail>,
) {
    use dx_update::outcome::SetStatus;
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<dx_update::sets::SetId, SetDetail> = BTreeMap::new();
    for (set, request) in resolved {
        let plan = match dx_update::backend::plan(workspace, *set, request, offline) {
            Ok(plan) => plan,
            Err(error) => {
                match error {
                    dx_update::backend::BackendError::Unsupported { reason, .. } => {
                        record_outcome(
                            &mut attempted,
                            &mut details,
                            *set,
                            SetStatus::Unsupported,
                            SetDetail::Failed {
                                message: format!("unsupported update: {reason}"),
                            },
                        );
                    }
                    dx_update::backend::BackendError::OfflineRequired { .. } => {
                        record_outcome(
                            &mut attempted,
                            &mut details,
                            *set,
                            SetStatus::Failed,
                            SetDetail::Failed {
                                message: format!("{error}"),
                            },
                        );
                    }
                }
                continue;
            }
        };
        match plan {
            dx_update::backend::BackendPlan::Noop => {
                record_outcome(
                    &mut attempted,
                    &mut details,
                    *set,
                    SetStatus::ManualPinned,
                    SetDetail::Note {
                        message: pinned_line(*set, request),
                    },
                );
            }
            dx_update::backend::BackendPlan::Run { argv, env: extra } => {
                run_update_backend(BackendRun {
                    attempted: &mut attempted,
                    details: &mut details,
                    set: *set,
                    request,
                    runner,
                    workspace,
                    argv: &argv,
                    extra: &extra,
                });
            }
        }
    }
    (attempted, details)
}

struct BackendRun<'a> {
    attempted: &'a mut Vec<dx_update::outcome::SetOutcome>,
    details: &'a mut BTreeMap<dx_update::sets::SetId, SetDetail>,
    set: dx_update::sets::SetId,
    request: &'a dx_update::selector::SetRequest,
    runner: &'a dyn dx_process::Runner,
    workspace: &'a std::path::Path,
    argv: &'a [String],
    extra: &'a [(String, String)],
}

fn run_update_backend(run: BackendRun<'_>) {
    use dx_update::outcome::SetStatus;
    let BackendRun {
        attempted,
        details,
        set,
        request,
        runner,
        workspace,
        argv,
        extra,
    } = run;
    let env_refs: Vec<(&str, &str)> = extra
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    match runner.run(argv, workspace, &env_refs) {
        Err(error) => {
            record_outcome(
                attempted,
                details,
                set,
                SetStatus::Failed,
                SetDetail::Failed {
                    message: format!(
                        "failed to update {}: failed to launch updater: {error}",
                        set.name()
                    ),
                },
            );
        }
        Ok(status) => match status.code {
            Some(0) => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Updated,
                    SetDetail::Note {
                        message: updated_line(set, request),
                    },
                );
            }
            Some(code) => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Failed,
                    SetDetail::Failed {
                        message: format!("failed to update {}: updater exited {code}", set.name()),
                    },
                );
            }
            None => {
                record_outcome(
                    attempted,
                    details,
                    set,
                    SetStatus::Failed,
                    SetDetail::Failed {
                        message: format!(
                            "failed to update {}: updater terminated by signal",
                            set.name()
                        ),
                    },
                );
            }
        },
    }
}

fn emit_update_json(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    verbose: bool,
    exit: i32,
) -> i32 {
    for outcome in &report.outcomes {
        let set_name = outcome.set.as_str();
        let correlation = format!("update:{set_name}");
        match outcome.status {
            dx_update::outcome::ReportedStatus::Updated => {
                emit_note(
                    out,
                    &correlation,
                    "update_set_success",
                    "info",
                    note_message(details, set_name, &format!("updated {set_name}")),
                    set_name,
                );
                if let Some(manifest) = live_success_manifest(set_name) {
                    if let Ok(file_events) = project_manifest_events(&manifest) {
                        for file_event in &file_events {
                            let _ = write_event(out, file_event);
                        }
                    }
                }
            }
            dx_update::outcome::ReportedStatus::Current => {
                emit_note(
                    out,
                    &correlation,
                    "update_set_current",
                    "info",
                    note_message(details, set_name, &format!("{set_name} is current")),
                    set_name,
                );
            }
            dx_update::outcome::ReportedStatus::ManualPinned => {
                emit_note(
                    out,
                    &correlation,
                    "update_set_pinned",
                    "info",
                    note_message(details, set_name, &format!("{set_name} is pinned")),
                    set_name,
                );
            }
            dx_update::outcome::ReportedStatus::Unsupported => {
                let message = failed_message(details, set_name, &format!("unsupported {set_name}"));
                if let Ok(event) =
                    error_event("update_unsupported", &message, None, None, Some("execute"))
                {
                    let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
                    let _ = write_event(out, &event);
                }
                let _ = writeln!(err, "dx: update_unsupported: {message}");
            }
            dx_update::outcome::ReportedStatus::Unavailable => {
                emit_note(
                    out,
                    &correlation,
                    "update_set_unavailable",
                    "warning",
                    note_message(details, set_name, &format!("{set_name} is unverified")),
                    set_name,
                );
            }
            dx_update::outcome::ReportedStatus::Failed => {
                let message =
                    failed_message(details, set_name, &format!("failed to update {set_name}"));
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
    emit_recovery(out, err, report);
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

fn emit_note(
    out: &mut dyn std::io::Write,
    correlation: &str,
    code: &str,
    level: &str,
    message: String,
    set_name: &str,
) {
    if let Ok(event) = notice_event(&NoticeEvent {
        level: level.to_owned(),
        code: code.to_owned(),
        message,
        related_command: Some("update".to_owned()),
        scope: Some(vec![set_name.to_owned()]),
        path: None,
        language: None,
        import: None,
    }) {
        let event = with_correlation(event.clone(), correlation).unwrap_or(event);
        let _ = write_event(out, &event);
    }
}

fn emit_recovery(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
) {
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
}

fn emit_update_text(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    report: &dx_update::outcome::UpdateReport,
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    verbose: bool,
    exit: i32,
) -> i32 {
    use dx_update::outcome::ReportedStatus;
    for outcome in &report.outcomes {
        match outcome.status {
            ReportedStatus::Updated
            | ReportedStatus::Current
            | ReportedStatus::ManualPinned
            | ReportedStatus::Unavailable => {
                if verbose {
                    if let Some(SetDetail::Note { message }) =
                        details.get(&parse_set(outcome.set.as_str()))
                    {
                        let _ = writeln!(out, "{message}");
                    }
                }
            }
            ReportedStatus::Unsupported => {
                if let Some(SetDetail::Failed { message }) =
                    details.get(&parse_set(outcome.set.as_str()))
                {
                    let _ = writeln!(err, "dx: update_unsupported: {message}");
                }
            }
            ReportedStatus::Failed => {
                if let Some(SetDetail::Failed { message }) =
                    details.get(&parse_set(outcome.set.as_str()))
                {
                    let code = if message.contains(CODE_OFFLINE_REQUIRED) {
                        CODE_OFFLINE_REQUIRED
                    } else {
                        CODE_UPDATE_FAILED
                    };
                    let _ = writeln!(err, "dx: {code}: {message}");
                }
            }
            ReportedStatus::Blocked => {
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
        let succeeded = report.successes().len();
        let failed = report.failures().len();
        let blocked = report.blocked().len();
        let unavailable = report.unavailable().len();
        if unavailable == 0 {
            let _ = writeln!(
                out,
                "dx update: {succeeded} succeeded, {failed} failed, {blocked} blocked"
            );
        } else {
            let _ = writeln!(
                out,
                "dx update: {succeeded} succeeded, {failed} failed, {blocked} blocked, {unavailable} unavailable"
            );
        }
    }
    exit
}

enum SetDetail {
    Note { message: String },
    Failed { message: String },
}

fn note_message(
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    set_name: &str,
    fallback: &str,
) -> String {
    details
        .get(&parse_set(set_name))
        .and_then(|detail| match detail {
            SetDetail::Note { message } => Some(message.clone()),
            SetDetail::Failed { .. } => None,
        })
        .unwrap_or_else(|| fallback.to_owned())
}

fn failed_message(
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    set_name: &str,
    fallback: &str,
) -> String {
    details
        .get(&parse_set(set_name))
        .and_then(|detail| match detail {
            SetDetail::Failed { message } => Some(message.clone()),
            SetDetail::Note { .. } => None,
        })
        .unwrap_or_else(|| fallback.to_owned())
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

fn check_summary(
    resolved: &BTreeMap<dx_update::sets::SetId, dx_update::selector::SetRequest>,
    checking: bool,
) -> String {
    let all_full = resolved.len() == dx_update::sets::SetId::ALL.len()
        && resolved
            .values()
            .all(|request| *request == dx_update::selector::SetRequest::Full);
    if all_full {
        if checking {
            return "Checking all dependency sets".to_owned();
        }
        return "Running update for all dependency sets".to_owned();
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
    if checking {
        format!("Checking {}", parts.join(", "))
    } else {
        format!("Running update for {}", parts.join(", "))
    }
}

fn current_line(set: dx_update::sets::SetId) -> String {
    let locks = set.locks().join(", ");
    format!("{name} is current ({locks})", name = set.name())
}

fn stale_line(set: dx_update::sets::SetId) -> String {
    let locks = set.locks().join(", ");
    format!(
        "{name} is stale ({locks}); run `dx update --apply {name}` to refresh",
        name = set.name()
    )
}

fn pinned_line(set: dx_update::sets::SetId, request: &dx_update::selector::SetRequest) -> String {
    if *request != dx_update::selector::SetRequest::Full {
        let locks = set.locks().join(", ");
        return format!(
            "{name} is pinned ({locks}); left untouched",
            name = set.name()
        );
    }
    match set {
        dx_update::sets::SetId::Go => {
            "go is pinned (third_party/go/go.mod, third_party/go/go.sum); left untouched (pins track Gazelle; widen explicitly via `dx bump gomod:<module> <version>`)".to_owned()
        }
        dx_update::sets::SetId::Ruby => {
            "ruby is pinned (third_party/ruby/Gemfile.lock, examples/adopt-ruby/Gemfile.lock); left untouched (regenerate with `bundle lock` on the seed host)".to_owned()
        }
        dx_update::sets::SetId::PowerShell => {
            "powershell is pinned (third_party/powershell/PSGallery.lock.json); left untouched (hand-regenerate from exact requirements; builds never run Install-Module)".to_owned()
        }
        _ => {
            let locks = set.locks().join(", ");
            format!(
                "{name} is pinned ({locks}); left untouched",
                name = set.name()
            )
        }
    }
}

fn updated_line(set: dx_update::sets::SetId, request: &dx_update::selector::SetRequest) -> String {
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
            | dx_update::sets::SetId::PowerShell => pinned_line(set, request),
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
