use super::common::*;
use super::quality_apply::{apply_collected_changes, project_status};
use super::quality_baseline::{
    apply_baseline, load_baseline, refresh_entries, write_refresh, BaselineInput,
    BaselineReport, CODE_BASELINE_INVALID, CODE_BASELINE_REFRESH_FAILED, CODE_BASELINE_STALE,
};
use super::quality_emit::{emit_findings, EmitInputs};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{write_standard_reports, StandardReports};
use super::results::collect_results;
use crate::args::Invocation;
use crate::plan::{bep_path, plan_build};
use crate::reports::{plan_reports, Destination};
use crate::resolve::resolve;
use dx_output::{
    command_finished, command_started, meets_threshold, write_event, FinishedCounts, OutputMode,
    Severity,
};

pub(crate) fn execute_quality(invocation: &Invocation, env: Env<'_>) -> i32 {
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
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str() else {
        return operational(
            invocation,
            out,
            err,
            CODE_UNREADABLE_BEP,
            "temporary event path is not UTF-8",
        );
    };
    let build = match resolve(
        &invocation.targets,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    )
    .map_err(|error| error.to_string())
    .and_then(|resolved| {
        plan_build(
            invocation.command,
            &resolved,
            &invocation.bazel_options,
            bep_text,
            &invocation.bazel_startup_options,
        )
        .map_err(|error| format!("{error}"))
    }) {
        Ok(build) => build,
        Err(message) => return pre_exec(err, &message),
    };
    let apply = invocation.applies();
    let mode = if apply { "default" } else { "check" };
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, mode) {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
        } else if invocation.chatty() && !stdout_report {
            let _ = writeln!(out, "{}", build.summary);
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, mode) {
            let _ = write_event(out, &event);
        }
    } else if invocation.chatty() && !stdout_report {
        let _ = writeln!(out, "{}", build.summary);
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &build.argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    let mut collected = match collect_results(&bep, workspace) {
        Ok(collected) => collected,
        Err((code, message)) => {
            let _ = std::fs::remove_file(&bep);
            return operational(invocation, out, err, &code, &message);
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
    let (status, mut failed) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &applied,
        invocation.fail_on,
        !collected.changes.is_empty(),
    );

    let mut suppressed = vec![false; status.len()];
    let mut baseline_report: Option<BaselineReport> = None;
    let mut baseline_failed = false;
    if let Some(rel) = invocation.baseline.as_deref() {
        match load_baseline(workspace, rel) {
            Err(detail) => {
                let _ = writeln!(err, "dx: {CODE_BASELINE_INVALID}: {detail}");
                if invocation.output == OutputMode::Json {
                    if let Ok(event) =
                        dx_output::error_event(CODE_BASELINE_INVALID, &detail, None, None, None)
                    {
                        let _ = write_event(out, &event);
                    }
                }
                baseline_failed = true;
            }
            Ok(input) => {
                let entries = match input {
                    BaselineInput::Absent => Vec::new(),
                    BaselineInput::Entries(entries) => entries,
                };
                let outcome = apply_baseline(
                    workspace,
                    &status,
                    &collected.analyzed,
                    collected.complete,
                    &entries,
                );
                let threshold_before = status
                    .iter()
                    .any(|diagnostic| meets_threshold(diagnostic.severity, invocation.fail_on));
                let threshold_new = status
                    .iter()
                    .zip(outcome.suppressed.iter())
                    .filter(|(_, suppressed)| !**suppressed)
                    .map(|(diagnostic, _)| diagnostic)
                    .any(|diagnostic| meets_threshold(diagnostic.severity, invocation.fail_on));
                failed = (failed && !threshold_before) || threshold_new;
                if !outcome.stale_indices.is_empty() {
                    failed = true;
                    let detail = format!("{rel} has {} stale entries", outcome.stale_indices.len());
                    let _ = writeln!(err, "dx: {CODE_BASELINE_STALE}: {detail}");
                    if invocation.output == OutputMode::Json {
                        if let Ok(event) =
                            dx_output::error_event(CODE_BASELINE_STALE, &detail, None, None, None)
                        {
                            let _ = write_event(out, &event);
                        }
                    }
                }
                suppressed = outcome.suppressed.clone();
                let mut refreshed = None;
                if apply && collected.complete {
                    let refresh = refresh_entries(
                        &entries,
                        &outcome.stale_indices,
                        &outcome.new_fingerprints,
                    )
                    .and_then(|refresh| {
                        write_refresh(workspace, rel, &refresh.bytes)
                            .map(|()| (refresh.pruned, refresh.added))
                    });
                    match refresh {
                        Ok(counts) => {
                            refreshed = Some(counts);
                        }
                        Err(detail) => {
                            let _ = writeln!(err, "dx: {CODE_BASELINE_REFRESH_FAILED}: {detail}");
                            if invocation.output == OutputMode::Json {
                                if let Ok(event) = dx_output::error_event(
                                    CODE_BASELINE_REFRESH_FAILED,
                                    &detail,
                                    None,
                                    None,
                                    None,
                                ) {
                                    let _ = write_event(out, &event);
                                }
                            }
                            baseline_failed = true;
                        }
                    }
                }
                let mut stale = Vec::new();
                for index in &outcome.stale_indices {
                    if let Some(entry) = entries.get(*index) {
                        stale.push(entry.clone());
                    }
                }
                baseline_report = Some(BaselineReport {
                    rel: rel.to_owned(),
                    total: outcome.total,
                    suppressed_count: outcome.suppressed_count,
                    new_count: outcome.new_count,
                    refreshed,
                    stale,
                });
            }
        }
    }

    let mut patch = String::new();
    if invocation.output == OutputMode::Diff {
        match render_diff_patch(&sources, &collected.changes) {
            Ok(rendered) => patch = rendered,
            Err(error) => {
                return operational(invocation, out, err, CODE_DIFF_FAILED, &error.to_string());
            }
        }
    }

    let emit_counts = emit_findings(
        EmitInputs {
            invocation,
            status: &status,
            suppressed: &suppressed,
            baseline: baseline_report.as_ref(),
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

    let mut reports_ok = write_standard_reports(
        StandardReports {
            workspace,
            collected: &collected,
            status: &status,
            suppressed: &suppressed,
            planned: &planned_reports,
            output: &invocation.output,
            stdout_report,
        },
        out,
        err,
    );
    if baseline_failed {
        reports_ok = false;
    }

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
    if invocation.output == OutputMode::Json {
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
