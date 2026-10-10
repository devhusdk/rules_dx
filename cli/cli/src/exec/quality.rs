use super::common::*;
use super::quality_apply::{apply_collected_changes, project_status};
use super::quality_emit::{emit_findings, EmitInputs};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{write_standard_reports, StandardReports};
use super::results::collect_results;
use crate::args::{Invocation, QualityRequest};
use crate::plan::{bep_path, plan_build};
use crate::reports::{plan_reports, Destination};
use crate::resolve::resolve;
use dx_output::{
    command_finished, command_started, write_event, FinishedCounts, OutputMode, Severity,
};

pub(crate) fn execute_quality(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match QualityRequest::from_invocation(invocation) {
        Ok(request) => request,
        Err(message) => return pre_exec(env.err, &message),
    };
    execute_quality_request(&request, env)
}

pub(crate) fn execute_quality_request(request: &QualityRequest, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ci: _,
    } = env;
    let planned_reports = match plan_reports(
        workspace,
        request.command,
        &request.common.reports,
        &request.common.output,
        request.dry_run,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str() else {
        return operational_for_output(
            request.common.output,
            out,
            err,
            CODE_UNREADABLE_BEP,
            "temporary event path is not UTF-8",
        );
    };
    let build = match resolve(
        &request.targets,
        workspace,
        query_runner,
        &request.common.bazel_startup_options,
    )
    .map_err(|error| error.to_string())
    .and_then(|resolved| {
        plan_build(
            request.command,
            &resolved,
            &request.bazel_options,
            bep_text,
            &request.common.bazel_startup_options,
        )
        .map_err(|error| format!("{error}"))
    }) {
        Ok(build) => build,
        Err(message) => return pre_exec(err, &message),
    };
    let apply = request.applies();
    let mode = if apply { "default" } else { "check" };
    if request.dry_run {
        if request.common.output == OutputMode::Json {
            if let Ok(event) = command_started(request.command.name(), true, mode) {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
        } else if request.chatty() && !stdout_report {
            let _ = writeln!(out, "{}", build.summary);
        }
        return 0;
    }
    if request.common.output == OutputMode::Json {
        if let Ok(event) = command_started(request.command.name(), false, mode) {
            let _ = write_event(out, &event);
        }
    } else if request.chatty() && !stdout_report {
        let _ = writeln!(out, "{}", build.summary);
    }
    let bazel_code = match run_bazel_for_output(
        request.common.output,
        out,
        err,
        workspace,
        runner,
        &build.argv,
        &[],
    ) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    let mut collected = match collect_results(&bep, workspace) {
        Ok(collected) => collected,
        Err((code, message)) => {
            let _ = std::fs::remove_file(&bep);
            return operational_for_output(request.common.output, out, err, &code, &message);
        }
    };
    let _ = std::fs::remove_file(&bep);
    collected.complete = collected.complete && bazel_code == 0;
    dx_output::sort_diagnostics(&mut collected.initial);
    dx_output::sort_diagnostics(&mut collected.terminal);
    collected
        .changes
        .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    let applied_outcome =
        apply_collected_changes(workspace, apply, collected.complete, &collected.changes);
    let sources = applied_outcome.sources;
    let applied = applied_outcome.applied;
    let not_applied = applied_outcome.not_applied;
    let (status, failed) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &applied,
        request.fail_on,
        !collected.changes.is_empty(),
    );

    let mut patch = String::new();
    if request.common.output == OutputMode::Diff {
        match render_diff_patch(&sources, &collected.changes) {
            Ok(rendered) => patch = rendered,
            Err(error) => {
                return operational_for_output(
                    request.common.output,
                    out,
                    err,
                    CODE_DIFF_FAILED,
                    &error.to_string(),
                );
            }
        }
    }

    let emit_counts = emit_findings(
        EmitInputs {
            output: request.common.output,
            apply,
            status: &status,
            changes: &collected.changes,
            applied: &applied,
            not_applied: &not_applied,
            patch: &patch,
            stdout_report,
        },
        out,
        err,
    );
    let applied_count = emit_counts.applied_count;
    let not_applied_count = emit_counts.not_applied_count;
    let change_count = collected.changes.len() as u64;

    let reports_ok = write_standard_reports(
        StandardReports {
            workspace,
            collected: &collected,
            status: &status,
            planned: &planned_reports,
            output: &request.common.output,
            stdout_report,
        },
        out,
        err,
    );

    let mut info = 0u64;
    let mut warning = 0u64;
    let mut error = 0u64;
    for diagnostic in &status {
        match diagnostic.severity {
            Severity::Info => info += 1,
            Severity::Warning => warning += 1,
            Severity::Error => error += 1,
        }
    }
    if request.common.output == OutputMode::Json {
        let finished = command_finished(
            if collected.complete && !failed && reports_ok {
                0
            } else {
                1
            },
            &FinishedCounts {
                results_complete: Some(collected.complete),
                diagnostics: Some([info, warning, error]),
                changes: Some([0, change_count]),
                mutations: if apply {
                    Some([applied_count, not_applied_count])
                } else {
                    None
                },
            },
        );
        let _ = write_event(out, &finished);
    }
    if collected.complete && !failed && reports_ok {
        0
    } else {
        1
    }
}

#[cfg(test)]
#[path = "quality_edits.rs"]
mod quality_edits;
#[cfg(test)]
#[path = "quality_modes.rs"]
mod quality_modes;
#[cfg(test)]
#[path = "quality_reporting.rs"]
mod quality_reporting;
