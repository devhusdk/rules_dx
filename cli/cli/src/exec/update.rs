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
    let verbose =
        matches!(invocation.output, OutputMode::Text { quiet: false }) && !invocation.quiet;
    if invocation.check {
        let Env {
            workspace,
            out,
            err,
            ..
        } = env;
        let mode = "check";
        if invocation.dry_run {
            if invocation.output == OutputMode::Json {
                if let Ok(event) = command_started(invocation.command.name(), true, mode) {
                    let _ = write_event(out, &event);
                }
                let finished = command_finished(0, &FinishedCounts::default());
                let _ = write_event(out, &finished);
            } else if verbose {
                let _ = writeln!(out, "Would check preset fragment");
            }
            return 0;
        }
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), false, mode) {
                let _ = write_event(out, &event);
            }
        } else if verbose {
            let _ = writeln!(out, "Running update --check for preset");
        }
        match dx_adopt::check_preset(workspace) {
            Ok(()) => {
                if invocation.output == OutputMode::Json {
                    let finished = command_finished(0, &FinishedCounts::default());
                    let _ = write_event(out, &finished);
                } else if verbose {
                    let _ = writeln!(out, "preset clean");
                }
                0
            }
            Err(dx_adopt::PresetError::Stale { detail }) => {
                if invocation.output == OutputMode::Json {
                    if let Ok(event) =
                        error_event(CODE_UPDATE_FAILED, &detail, None, None, Some("execute"))
                    {
                        let _ = write_event(out, &event);
                    }
                    let finished = command_finished(1, &FinishedCounts::default());
                    let _ = write_event(out, &finished);
                } else {
                    let _ = writeln!(out, "{detail}");
                }
                1
            }
            Err(error) => operational(invocation, out, err, CODE_UPDATE_FAILED, &error.to_string()),
        }
    } else {
        execute_update_default(invocation, env, verbose)
    }
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
    let mut summary = display_summary(&resolved);
    if invocation.offline {
        summary.push_str(" (offline, cache-only)");
    }
    if invocation.dry_run {
        return emit_update_dry_run(invocation, out, &summary, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    if let Err(error) = dx_adopt::update_preset(workspace) {
        return operational(invocation, out, err, CODE_UPDATE_FAILED, &error.to_string());
    }
    if verbose && invocation.output != OutputMode::Json {
        let _ = writeln!(out, "updated preset (tools/bazelrc/preset.bazelrc)");
    }
    let (attempted, details) =
        run_update_backends(&resolved, runner, workspace, invocation.offline);
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
        emit_update_json(out, err, &report, &details, verbose, exit)
    } else {
        emit_update_text(out, err, &report, &details, verbose, exit)
    }
}

fn emit_update_dry_run(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    summary: &str,
    verbose: bool,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), true, "default") {
            let _ = write_event(out, &event);
        }
        let finished = command_finished(0, &FinishedCounts::default());
        let _ = write_event(out, &finished);
    } else if verbose {
        let _ = writeln!(out, "{summary}");
        let _ = writeln!(out, "Would update preset fragment");
    }
    0
}

fn record_set_failed(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<dx_update::sets::SetId, SetDetail>,
    set: dx_update::sets::SetId,
    message: String,
) {
    attempted.push(dx_update::outcome::SetOutcome {
        set: set.name().to_owned(),
        status: dx_update::outcome::SetStatus::Failed,
    });
    details.insert(set, SetDetail::Failed { message });
}

fn record_set_success(
    attempted: &mut Vec<dx_update::outcome::SetOutcome>,
    details: &mut BTreeMap<dx_update::sets::SetId, SetDetail>,
    set: dx_update::sets::SetId,
    message: String,
) {
    attempted.push(dx_update::outcome::SetOutcome {
        set: set.name().to_owned(),
        status: dx_update::outcome::SetStatus::Success,
    });
    details.insert(set, SetDetail::Success { message });
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
    let mut attempted: Vec<dx_update::outcome::SetOutcome> = Vec::new();
    let mut details: BTreeMap<dx_update::sets::SetId, SetDetail> = BTreeMap::new();
    for (set, request) in resolved {
        let plan = match dx_update::backend::plan(*set, request, offline) {
            Ok(plan) => plan,
            Err(error) => {
                match error {
                    dx_update::backend::BackendError::Unsupported { reason, .. } => {
                        let detail = format!("unsupported update: {reason}");
                        record_set_failed(
                            &mut attempted,
                            &mut details,
                            *set,
                            format!("failed to update {}: {detail}", set.name()),
                        );
                    }
                    dx_update::backend::BackendError::OfflineRequired { .. } => {
                        record_set_failed(
                            &mut attempted,
                            &mut details,
                            *set,
                            format!("failed to update {}: {error}", set.name()),
                        );
                    }
                }
                continue;
            }
        };
        match plan {
            dx_update::backend::BackendPlan::Noop => {
                record_set_success(
                    &mut attempted,
                    &mut details,
                    *set,
                    success_line(*set, request),
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
            record_set_failed(
                attempted,
                details,
                set,
                format!(
                    "failed to update {}: failed to launch updater: {error}",
                    set.name()
                ),
            );
        }
        Ok(status) => match status.code {
            Some(0) => {
                record_set_success(attempted, details, set, success_line(set, request));
            }
            Some(code) => {
                record_set_failed(
                    attempted,
                    details,
                    set,
                    format!("failed to update {}: updater exited {code}", set.name()),
                );
            }
            None => {
                record_set_failed(
                    attempted,
                    details,
                    set,
                    format!(
                        "failed to update {}: updater terminated by signal",
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
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    verbose: bool,
    exit: i32,
) -> i32 {
    for outcome in &report.outcomes {
        let set_name = outcome.set.as_str();
        let correlation = format!("update:{set_name}");
        match outcome.status {
            dx_update::outcome::ReportedStatus::Success => {
                let message = details
                    .get(&parse_set(set_name))
                    .and_then(|detail| match detail {
                        SetDetail::Success { message } => Some(message.clone()),
                        SetDetail::Failed { .. } => None,
                    })
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
            dx_update::outcome::ReportedStatus::Failed => {
                let message = details
                    .get(&parse_set(set_name))
                    .and_then(|detail| match detail {
                        SetDetail::Failed { message } => Some(message.clone()),
                        SetDetail::Success { .. } => None,
                    })
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
    details: &BTreeMap<dx_update::sets::SetId, SetDetail>,
    verbose: bool,
    exit: i32,
) -> i32 {
    for outcome in &report.outcomes {
        match outcome.status {
            dx_update::outcome::ReportedStatus::Success => {
                if verbose {
                    if let Some(SetDetail::Success { message }) =
                        details.get(&parse_set(outcome.set.as_str()))
                    {
                        let _ = writeln!(out, "{message}");
                    }
                }
            }
            dx_update::outcome::ReportedStatus::Failed => {
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
        let succeeded = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == dx_update::outcome::ReportedStatus::Success)
            .count();
        let failed = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == dx_update::outcome::ReportedStatus::Failed)
            .count();
        let blocked = report
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == dx_update::outcome::ReportedStatus::Blocked)
            .count();
        let _ = writeln!(
            out,
            "dx update: {succeeded} succeeded, {failed} failed, {blocked} blocked"
        );
    }
    exit
}

enum SetDetail {
    Success { message: String },
    Failed { message: String },
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
) -> String {
    let all_full = resolved.len() == dx_update::sets::SetId::ALL.len()
        && resolved
            .values()
            .all(|request| *request == dx_update::selector::SetRequest::Full);
    if all_full {
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
    format!("Running update for {}", parts.join(", "))
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
            dx_update::sets::SetId::Go => {
                "updated go (pinned module lock; no-op success)".to_owned()
            }
            dx_update::sets::SetId::Ruby => {
                "updated ruby (third_party/ruby/Gemfile.lock; no-op success)".to_owned()
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
