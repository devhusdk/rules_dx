use super::common::*;
use super::dep_edit;
use super::update_version_add;
use dx_error::{DxError, ErrorKind};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, command_started, notice_event,
};
use dx_plan::args::Invocation;
use dx_plan::reports::plan_reports;
pub use dx_update::edit::gha::COMMIT_JSON_ENV;

pub(crate) fn execute_update_version(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.edits_version(),
        "update dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        edit_commit_json,
        ..
    } = env;
    match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    }
    if invocation.targets.len() != 2 {
        return pre_exec(
            invocation.output,
            out,
            err,
            "update needs exactly <selector> <version>",
        );
    }
    let request = match configured_request(invocation, out, err, workspace) {
        Ok(Some(request)) => request,
        Ok(None) => {
            let selector = &invocation.targets[0];
            let head = selector
                .split_once(':')
                .map_or(selector.as_str(), |(h, _)| h);
            if !head.is_empty()
                && !invocation.targets[1].is_empty()
                && !dx_update::edit::UpdateSet::builtin_names().contains(&head)
            {
                let message = dep_edit::unknown_set_error(workspace, head);
                return pre_exec(invocation.output, out, err, &message);
            }
            match dx_update::edit::UpdateRequest::parse(
                &invocation.targets[0],
                &invocation.targets[1],
            ) {
                Ok(request) => request,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            }
        }
        Err(exit) => return exit,
    };
    let request = match dx_update::edit::DepVersion::from_flags(
        invocation.floor,
        invocation.reason.as_deref(),
    )
    .and_then(|target| request.with_dep_version(target))
    {
        Ok(request) => request,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let lazy_fetch = |url: &str| super::update_outdated::registry_fetch().get(url);
    let request = match resolve_gha_tag_request(
        &request,
        edit_commit_json,
        invocation.offline,
        &lazy_fetch,
    ) {
        Ok(request) => request,
        Err((code, error)) => {
            return report_dx_error(invocation.output, out, err, code, &error);
        }
    };
    let summary = frozen_summary(
        offline_summary(request.summary(), invocation.offline),
        invocation.frozen,
    );
    let verbose = invocation.chatty();
    if !invocation.applies() {
        return check_edit(
            invocation,
            update_version_add::Io {
                workspace,
                runner,
                query_runner,
                out,
                err,
            },
            &request,
            verbose,
        );
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
    } else if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
        return exit;
    }
    let planned = match plan_widens(&request, workspace) {
        Ok(planned) => planned,
        Err(WidenFailure::Usage(message)) => {
            return pre_exec(invocation.output, out, err, &message);
        }
        Err(WidenFailure::Missing(message)) => {
            let io = update_version_add::Io {
                workspace,
                runner,
                query_runner,
                out,
                err,
            };
            return update_version_add::add_missing(invocation, io, &request, &message);
        }
        Err(WidenFailure::Failed(error)) => {
            return report_dx_error(
                invocation.output,
                out,
                err,
                CODE_UPDATE_WIDEN_FAILED,
                &error,
            );
        }
    };
    if planned.iter().all(|(_, _, changed)| !changed) {
        return emit_update_no_change(invocation, out, &request, &planned, verbose);
    }
    let target = request.target();
    if let Err(exit) = dep_edit::preflight(
        &EDIT_VERB,
        invocation,
        out,
        err,
        workspace,
        &target,
        &request.selector,
    ) {
        return exit;
    }
    let changed: Vec<(String, String)> = planned
        .into_iter()
        .filter(|(_, _, changed)| *changed)
        .map(|(manifest, text, _)| (manifest, text))
        .collect();
    let manifest = changed
        .iter()
        .map(|(manifest, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let edit = dep_edit::DepEdit {
        verb: &EDIT_VERB,
        target: &target,
        selector: &request.selector,
        edited: format!(
            "widened {} to {} in {manifest}",
            request.selector,
            request.version.display()
        ),
        kept: format!("widen kept in {manifest}"),
    };
    let io = dep_edit::EditIo {
        workspace,
        runner,
        query_runner,
        out,
        err,
    };
    dep_edit::write_and_refresh(&edit, invocation, io, &changed, verbose)
}

const EDIT_VERB: dep_edit::EditVerb = dep_edit::EditVerb {
    command: "update",
    noun: "widen",
    notice_code: CODE_UPDATE_WIDENED,
    failure_code: CODE_UPDATE_WIDEN_FAILED,
    noop_note: "pinned module lock; no-op success",
};

fn configured_request(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
) -> Result<Option<dx_update::edit::UpdateRequest>, i32> {
    let Some(configured) =
        dep_edit::configured_set(&EDIT_VERB, invocation, out, err, workspace, true)?
    else {
        return Ok(None);
    };
    dx_update::edit::UpdateRequest::parse_configured_set(
        configured.kind,
        &configured.name,
        &configured.manifests,
        configured.repository.as_deref(),
        &invocation.targets[0],
        &invocation.targets[1],
    )
    .map(Some)
    .map_err(|error| pre_exec(invocation.output, out, err, &error.to_string()))
}

fn check_edit(
    invocation: &Invocation,
    io: update_version_add::Io<'_>,
    request: &dx_update::edit::UpdateRequest,
    verbose: bool,
) -> i32 {
    let update_version_add::Io {
        workspace,
        runner,
        query_runner,
        out,
        err,
    } = io;
    if invocation.output == OutputMode::Json
        && let Err(exit) = emit_built(out, command_started(invocation.command.name(), "check"))
    {
        return exit;
    }
    let planned = match plan_widens(request, workspace) {
        Ok(planned) => planned,
        Err(WidenFailure::Usage(message)) => {
            return pre_exec(invocation.output, out, err, &message);
        }
        Err(WidenFailure::Missing(message)) => {
            let io = update_version_add::Io {
                workspace,
                runner,
                query_runner,
                out,
                err,
            };
            return update_version_add::add_missing(invocation, io, request, &message);
        }
        Err(WidenFailure::Failed(error)) => {
            return report_dx_error(
                invocation.output,
                out,
                err,
                CODE_UPDATE_WIDEN_FAILED,
                &error,
            );
        }
    };
    let mut drifted: Vec<String> = Vec::new();
    for (manifest, widened, _) in &planned {
        let current = dx_path::workspace::resolve_in_workspace(workspace, manifest)
            .and_then(std::fs::read)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        if current.as_deref() != Some(widened.as_str()) {
            drifted.push(manifest.clone());
        }
    }
    let manifest = planned
        .iter()
        .map(|(manifest, _, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if drifted.is_empty() {
        if planned.iter().all(|(_, _, changed)| !changed) {
            return emit_update_no_change(invocation, out, request, &planned, verbose);
        }
        let message = format!(
            "update {} to {} current in {manifest}",
            request.selector,
            request.version.display()
        );
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_built(
                out,
                notice_event(&NoticeEvent {
                    related_command: Some("update".to_owned()),
                    scope: Some(vec![request.selector.clone()]),
                    path: Some(manifest.clone()),
                    ..NoticeEvent::info(CODE_UPDATE_WIDENED, message.clone())
                }),
            ) {
                return exit;
            }
            let finished = command_finished(
                0,
                &FinishedCounts {
                    results_complete: Some(true),
                    ..FinishedCounts::default()
                },
            );
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
            return 0;
        }
        if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
            return exit;
        }
        return 0;
    }
    let message = format!(
        "update drift: {} needs widening to {} in {} (run `dx update {} {} --apply`)",
        request.selector,
        request.version.display(),
        drifted.join(", "),
        request.selector,
        request.version.display()
    );
    operational(invocation, out, err, CODE_UPDATE_WIDEN_FAILED, &message)
}

fn widen_targets(
    request: &dx_update::edit::UpdateRequest,
    workspace: &std::path::Path,
) -> Result<Vec<String>, DxError> {
    if let Some(manifests) = request.configured_manifests() {
        return Ok(manifests.to_vec());
    }
    if request.set != dx_update::edit::UpdateSet::GithubActions {
        return Ok(vec![request.target_manifest().to_owned()]);
    }
    dx_update::edit::gha::list_workflows(workspace).map_err(|error| {
        DxError::from_io(
            format!(
                "failed to widen {}: cannot list {}",
                request.selector,
                dx_update::edit::gha::WORKFLOW_DIR
            ),
            &error,
        )
    })
}

/// Why a widen could not be planned: a refused argument (exit 2) or a failed edit.
enum WidenFailure {
    Usage(String),
    Missing(String),
    Failed(DxError),
}

impl From<DxError> for WidenFailure {
    fn from(error: DxError) -> Self {
        WidenFailure::Failed(error)
    }
}

fn plan_widens(
    request: &dx_update::edit::UpdateRequest,
    workspace: &std::path::Path,
) -> Result<Vec<(String, String, bool)>, WidenFailure> {
    let mut planned = Vec::new();
    let targets = widen_targets(request, workspace)?;
    for manifest in &targets {
        let manifest = manifest.clone();
        let original = dx_path::workspace::resolve_in_workspace(workspace, &manifest)
            .and_then(std::fs::read)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::InvalidInput {
                    DxError::failure(format!(
                        "failed to widen {}: {manifest} leaves the workspace",
                        request.selector
                    ))
                } else {
                    DxError::from_io(
                        format!(
                            "failed to widen {}: cannot read {manifest}",
                            request.selector
                        ),
                        &error,
                    )
                }
            })?;
        let text = String::from_utf8(original).map_err(|_| {
            DxError::failure(format!(
                "failed to widen {}: {manifest} is not valid UTF-8",
                request.selector
            ))
        })?;
        match request.plan_edit_outcome(&manifest, &text) {
            Ok(dx_update::edit::EditPlanOutcome::Changed(widened)) => {
                planned.push((manifest, widened, true));
            }
            Ok(dx_update::edit::EditPlanOutcome::Unchanged) => {
                planned.push((manifest, text, false));
            }
            Err(dx_update::edit::EditError::NotFound { .. }) if request.set.spans_manifests() => {}
            Err(error @ dx_update::edit::EditError::NotFound { .. }) => {
                return Err(WidenFailure::Missing(format!(
                    "failed to widen {}: {error}",
                    request.selector
                )));
            }
            Err(error) if error.is_usage() => {
                return Err(WidenFailure::Usage(format!(
                    "failed to widen {}: {error}",
                    request.selector
                )));
            }
            Err(error) => {
                return Err(DxError::failure(format!(
                    "failed to widen {}: {error}",
                    request.selector
                ))
                .into());
            }
        }
    }
    if planned.is_empty() {
        let manifest = match request.configured_name() {
            Some(name) if request.set == dx_update::edit::UpdateSet::Cargo => {
                format!("any manifest of set {name}")
            }
            _ => request.target_manifest().to_owned(),
        };
        return Err(WidenFailure::Missing(format!(
            "failed to widen {}: {}",
            request.selector,
            dx_update::edit::EditError::NotFound {
                manifest,
                package: request.package.clone(),
            }
        )));
    }
    Ok(planned)
}

fn resolve_gha_tag_request(
    request: &dx_update::edit::UpdateRequest,
    commit_json: Option<&str>,
    offline: bool,
    fetch: &dyn dx_update::discover::Fetch,
) -> Result<dx_update::edit::UpdateRequest, (&'static str, DxError)> {
    let dx_update::edit::WidenVersion::GitTag(tag) = &request.version else {
        return Ok(request.clone());
    };
    if request.set != dx_update::edit::UpdateSet::GithubActions {
        return Ok(request.clone());
    }
    let failed = |kind: ErrorKind, reason: String| {
        (
            CODE_UPDATE_WIDEN_FAILED,
            DxError::new(
                kind,
                format!("failed to widen {}: {reason}", request.selector),
            ),
        )
    };
    let sha = if let Some(path) = commit_json {
        let text = std::fs::read_to_string(path).map_err(|error| {
            let kind = DxError::from_io("", &error).kind();
            failed(kind, format!(
                "cannot read {path} (nothing widened; unknown tags fail closed, never invent a SHA)"
            ))
        })?;
        dx_update::edit::gha::resolve_sha_from_commit_response(&text)
            .map_err(|error| failed(ErrorKind::Failure, error.to_string()))?
    } else if offline {
        return Err((
            CODE_OFFLINE_REQUIRED,
            DxError::failure(format!(
                "failed to widen {}: resolving tag {tag} needs the network; drop --offline or pass a commit SHA",
                request.selector
            )),
        ));
    } else {
        dx_update::discover::github::commit_sha(
            &request.package,
            tag,
            dx_update::discover::github::DEFAULT_BASE,
            fetch,
        )
        .map_err(|reason| failed(reason.kind(), format!("{reason} (nothing widened)")))?
    };
    request
        .with_resolved_sha(&sha)
        .map_err(|error| failed(ErrorKind::Failure, error.to_string()))
}

fn emit_update_no_change(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    request: &dx_update::edit::UpdateRequest,
    planned: &[(String, String, bool)],
    verbose: bool,
) -> i32 {
    let manifest = planned
        .iter()
        .map(|(manifest, _, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let message = format!(
        "{} already at {} in {manifest} (no-change; nothing widened)",
        request.selector,
        request.version.display()
    );
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("update".to_owned()),
                scope: Some(vec![request.selector.clone()]),
                path: Some(manifest),
                ..NoticeEvent::info(CODE_UPDATE_NO_CHANGE, message.clone())
            }),
        ) {
            return exit;
        }
        let finished = command_finished(
            0,
            &FinishedCounts {
                results_complete: Some(true),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
        return 0;
    }
    if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
        return exit;
    }
    0
}

#[cfg(test)]
#[path = "update_version_tests.rs"]
mod tests;
