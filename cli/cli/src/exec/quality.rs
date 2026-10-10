use super::common::*;
use super::quality_apply::{apply_collected_changes, project_status};
use super::quality_baseline::{apply_baseline, BaselineOutcome};
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
    let has_changes = !collected.changes.is_empty();
    let (status, failed) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &applied,
        invocation.fail_on,
        has_changes,
    );
    let baseline: Option<BaselineOutcome> = match &invocation.quality_baseline {
        None => None,
        Some(selection) => match apply_baseline(
            workspace,
            selection,
            apply,
            collected.complete,
            &collected.coverage,
            &status,
        ) {
            Ok(outcome) => Some(outcome),
            Err(detail) => {
                return operational(invocation, out, err, CODE_BASELINE_FAILED, &detail);
            }
        },
    };
    let failed = match &baseline {
        None => failed,
        Some(outcome) => {
            let new_failing = status.iter().enumerate().any(|(index, diagnostic)| {
                !outcome.suppressed.contains(&index)
                    && meets_threshold(diagnostic.severity, invocation.fail_on)
            });
            new_failing || !outcome.stale.is_empty() || (!apply && has_changes)
        }
    };

    let mut patch = String::new();
    if invocation.output == OutputMode::Diff {
        match render_diff_patch(&sources, &collected.changes) {
            Ok(rendered) => patch = rendered,
            Err(error) => {
                return operational(invocation, out, err, CODE_DIFF_FAILED, &error.to_string());
            }
        }
    }

    let no_suppressed = std::collections::BTreeSet::new();
    let suppressed = baseline
        .as_ref()
        .map(|outcome| &outcome.suppressed)
        .unwrap_or(&no_suppressed);
    let emit_counts = emit_findings(
        EmitInputs {
            invocation,
            status: &status,
            changes: &collected.changes,
            applied: &applied,
            not_applied: &not_applied,
            patch: &patch,
            stdout_report,
            suppressed,
            baseline: invocation
                .quality_baseline
                .as_deref()
                .zip(baseline.as_ref()),
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
            output: &invocation.output,
            stdout_report,
            suppressed,
            baselined: invocation.quality_baseline.is_some(),
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
