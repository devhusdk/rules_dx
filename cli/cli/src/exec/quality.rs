use super::common::*;
use super::quality_apply::{apply_collected_changes, project_status};
use super::quality_baseline::{apply_baseline, BaselineView};
use super::quality_emit::{emit_findings, EmitInputs};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{write_standard_reports, StandardReports};
use super::results::collect_results;
use crate::args::{Invocation, QualityRequest};
use crate::plan::{bep_path, plan_build, quality_provenance};
use crate::reports::{plan_reports, BaselineReport, Destination};
use crate::resolve::resolve;
use dx_output::{
    command_finished, command_started, meets_threshold, operation_event, write_event,
    FinishedCounts, OutputMode, Severity,
};

pub(crate) fn execute_quality(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.quality_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { err, .. } = env;
            return pre_exec(err, &message);
        }
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
        return operational_output(
            request.common.output,
            out,
            err,
            CODE_UNREADABLE_BEP,
            "temporary event path is not UTF-8",
        );
    };
    let resolved = match resolve(
        &request.targets,
        workspace,
        query_runner,
        &request.bazel_startup_options,
    )
    .map_err(|error| error.to_string())
    {
        Ok(resolved) => resolved,
        Err(message) => return pre_exec(err, &message),
    };
    let build = match plan_build(
        request.command,
        &resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
    )
    .map_err(|error| format!("{error}"))
    {
        Ok(build) => build,
        Err(message) => return pre_exec(err, &message),
    };
    let apply = request.applies();
    let mode = if apply { "default" } else { "check" };
    if request.dry_run {
        let provenance = quality_provenance(
            request.command,
            &resolved,
            &request.bazel_options,
            request.workspace.as_deref(),
            apply,
            &build.argv,
        );
        if request.common.output == OutputMode::Json {
            if let Ok(event) = command_started(request.command.name(), true, mode) {
                let _ = write_event(out, &event);
            }
            let scope_labels: Vec<String> = provenance["scope"]["targets"]
                .as_array()
                .map(|targets| {
                    targets
                        .iter()
                        .filter_map(|target| target.as_str().map(ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();
            if let Ok(serde_json::Value::Object(mut map)) =
                operation_event(request.command.name(), "plan", Some(&scope_labels))
            {
                map.insert("provenance".to_owned(), provenance);
                let _ = write_event(out, &serde_json::Value::Object(map));
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
        } else if request.chatty() && !stdout_report {
            let _ = writeln!(out, "{}", build.summary);
            let policy = provenance["policy"]["origin"].as_str().unwrap_or("default");
            let aspects = provenance["aspects"].as_array().map(Vec::len).unwrap_or(0);
            let _ = writeln!(out, "Policy: {policy}");
            let _ = writeln!(out, "Aspects: {aspects} selected");
            let _ = writeln!(
                out,
                "Execution: platform unknown, toolchain unknown (requires Bazel analysis)"
            );
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
    let bazel_code = match run_bazel_output(
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
            return operational_output(request.common.output, out, err, &code, &message);
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
    let (status, _) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &applied,
        request.fail_on,
        !collected.changes.is_empty(),
    );
    let baseline: Option<BaselineView> = match apply_baseline(
        workspace,
        &status,
        &collected.tools,
        &collected.terminal_digests,
        collected.complete,
        apply,
    ) {
        Ok(view) => view,
        Err((code, message)) => {
            return operational_output(request.common.output, out, err, &code, &message);
        }
    };
    let suppressed: Vec<bool> = baseline
        .as_ref()
        .map_or_else(|| vec![false; status.len()], |view| view.suppressed.clone());
    let mut failed =
        status.iter().zip(&suppressed).any(|(diagnostic, held)| {
            !held && meets_threshold(diagnostic.severity, request.fail_on)
        }) || baseline.as_ref().is_some_and(|view| !view.stale.is_empty());
    if !apply && !collected.changes.is_empty() {
        failed = true;
    }

    let mut patch = String::new();
    if request.common.output == OutputMode::Diff {
        match render_diff_patch(&sources, &collected.changes) {
            Ok(rendered) => patch = rendered,
            Err(error) => {
                return operational_output(
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
            request,
            status: &status,
            changes: &collected.changes,
            applied: &applied,
            not_applied: &not_applied,
            patch: &patch,
            stdout_report,
            suppressed: &suppressed,
            baseline: baseline.as_ref(),
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
            baseline: baseline.as_ref().map(|view| BaselineReport {
                file: view.rel.clone(),
                total: view.total,
                new: view.fresh,
                suppressed: view.held,
            }),
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
                baseline: baseline.as_ref().map(BaselineView::counts),
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
