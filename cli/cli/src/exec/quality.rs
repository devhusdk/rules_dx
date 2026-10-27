use super::build_failures::report_build_failures;
use super::common::*;
use super::guidance::{apply_hint, rerun_hint};
use super::quality_apply::{apply_collected_changes, project_status, terminal_on_disk};
use super::quality_baseline::{analyzed_scope, apply_baseline, BaselineGate, BaselineView};
use super::quality_emit::{emit_findings, EmitInputs};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{
    verified_snapshots, write_standard_reports, Snapshots, StandardReports,
};
use super::results::collect_results;
use dx_apply::{FileSystem, RealFileSystem};
use dx_output::{
    command_finished, command_started, meets_threshold, ActionCounts, FinishedCounts, OutputMode,
    Severity,
};
use dx_plan::args::{Command, Invocation, QualityRequest};
use dx_plan::plan::{bep_path, plan_build_with, plan_probe, SHARD_OUTPUT_GROUP};
use dx_plan::reports::{plan_reports, BaselineReport, Destination};
use dx_plan::resolve::resolve;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

pub(crate) fn execute_quality(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.quality_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { out, err, .. } = env;
            return pre_exec(invocation.output, out, err, &message);
        }
    };
    execute_quality_request(&request, env)
}

pub(crate) fn execute_quality_request(request: &QualityRequest, env: Env<'_>) -> i32 {
    run_quality(
        request,
        env,
        &RealFileSystem,
        &mut PhaseKind::Selection,
        &mut PhaseEffects::default(),
    )
}

pub(crate) fn execute_quality_phase(request: &QualityRequest, env: Env<'_>) -> PhaseRun {
    execute_quality_phase_with_fs(request, env, &RealFileSystem)
}

/// Runs one quality phase whose apply writes go through `fs`.
pub(crate) fn execute_quality_phase_with_fs(
    request: &QualityRequest,
    env: Env<'_>,
    fs: &dyn FileSystem,
) -> PhaseRun {
    let mut kind = PhaseKind::Selection;
    let mut effects = PhaseEffects::default();
    let code = run_quality(request, env, fs, &mut kind, &mut effects);
    PhaseRun {
        code,
        kind,
        effects,
    }
}

fn run_quality(
    request: &QualityRequest,
    env: Env<'_>,
    fs: &dyn FileSystem,
    kind: &mut PhaseKind,
    effects: &mut PhaseEffects,
) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ci,
        audit_today,
        actions,
        proc_root,
        bump_commit_json,
    } = env;
    let mut latched = LatchedOut::new(out);
    let code = run_quality_request(
        request,
        Env {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid,
            nonce,
            out: &mut latched,
            err,
            ci,
            audit_today,
            actions,
            proc_root,
            bump_commit_json,
        },
        fs,
        kind,
        effects,
    );
    latched.exit_code(code)
}

pub(super) fn operational_build_output(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    lines: &[String],
) -> i32 {
    let message = format!("build output in change: {}", lines.join("; "));
    report_failure(
        output,
        out,
        err,
        &Failure::new("build_output_in_change", &message, 1),
    )
}

pub(super) fn guard_build_output(
    workspace: &Path,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    match dx_adopt::build_output::scan(workspace) {
        Err(error) => {
            let message = format!("build-output check failed: {error}");
            Some(report_failure(
                output,
                out,
                err,
                &Failure::new(
                    "build_output_git_failed",
                    &message,
                    dx_process::pre_exec_code(),
                )
                .without_usage(),
            ))
        }
        Ok(report) => {
            if !report.findings.is_empty() {
                let lines: Vec<String> = report
                    .findings
                    .iter()
                    .map(|finding| format!("{}: {}", finding.path, finding.reason))
                    .collect();
                return Some(operational_build_output(output, out, err, &lines));
            }
            if output != OutputMode::Json {
                if let Some(note) = report.committed_skipped {
                    let _ = writeln!(err, "dx: build-output check: {note}");
                }
            }
            None
        }
    }
}

/// Fails `dx check` when the generated `third_party/rules_dx/` overrides are stale (#3736).
///
/// Only an error fails: a workspace that has not adopted the include (warn) gets a note on
/// stderr and continues, and rules_dx itself or a workspace without a rules_dx dep is silent.
pub(super) fn guard_overrides(
    workspace: &Path,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    match dx_adopt::check_overrides(workspace) {
        dx_adopt::OverridesState::Error(detail) => Some(report_failure(
            output,
            out,
            err,
            &Failure::new(
                "overrides_stale",
                &format!("rules_dx overrides: {detail}"),
                1,
            ),
        )),
        dx_adopt::OverridesState::Warn(detail) => {
            if output != OutputMode::Json {
                let _ = writeln!(err, "dx: overrides: {detail}");
            }
            None
        }
        dx_adopt::OverridesState::Ok | dx_adopt::OverridesState::NotApplicable(_) => None,
    }
}

/// Why a shard probe ended the phase instead of falling back to every shard.
#[derive(Debug, PartialEq, Eq)]
enum ProbeStop {
    /// Bazel died by a signal.
    Signalled,
    /// dx recorded a stop signal or Bazel exited as interrupted.
    Interrupted,
}

/// Runs the shard probe and returns the shards it named; `Ok(None)` means pass every shard.
///
/// Any failure (no probe for the command, a plan error, a non-zero exit, an unreadable or
/// incomplete event file, a malformed shard file) falls back to every shard, so the probe can
/// only make a run cheaper, never change its result. It writes nothing to stdout; Bazel's own
/// progress goes to stderr. A stop signal or interrupted Bazel ends the run instead.
///
/// The probe is a real build, so its action counts go into `actions` and into `probe_counts`
/// before its event file is removed (#2394).
#[allow(clippy::too_many_arguments)]
fn probe_shards(
    request: &QualityRequest,
    resolved: &dx_plan::resolve::ResolvedScope,
    workspace: &Path,
    runner: &dyn dx_process::Runner,
    temp_dir: &Path,
    pid: u32,
    nonce: u64,
    actions: &ActionTally,
    probe_counts: &mut Option<ActionCounts>,
) -> Result<Option<BTreeSet<String>>, ProbeStop> {
    let bep = temp_dir.join(format!("dx-bep-{pid}-{nonce}-probe.json"));
    let Some(bep_text) = bep.to_str() else {
        return Ok(None);
    };
    let plan = match plan_probe(
        request.command,
        resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
    ) {
        Ok(Some(plan)) => plan,
        Ok(None) | Err(_) => return Ok(None),
    };
    let status = runner.run(&plan.argv, workspace, &[]);
    *probe_counts = read_actions(&bep, actions);
    let shards = match &status {
        Ok(status) if status.code.is_none() => Err(ProbeStop::Signalled),
        _ if dx_process::interrupted().is_some() => Err(ProbeStop::Interrupted),
        Ok(status) if status.code == Some(BAZEL_INTERRUPTED) => Err(ProbeStop::Interrupted),
        Ok(status) if status.code == Some(0) => Ok(read_probe_shards(&bep, workspace)),
        _ => Ok(None),
    };
    let _ = std::fs::remove_file(&bep);
    shards
}

/// Unions the shard names of every `{"shards":[...]}` file in the probe's event file.
fn read_probe_shards(bep: &Path, workspace: &Path) -> Option<BTreeSet<String>> {
    let account = collect_output_account(bep, SHARD_OUTPUT_GROUP, workspace).ok()?;
    if account.problem.is_some() || !account.gaps.is_empty() || !account.ended {
        return None;
    }
    let mut shards = BTreeSet::new();
    for target in &account.targets {
        if !target.success || target.incomplete {
            return None;
        }
        for artifact in &target.artifacts {
            let value: serde_json::Value = serde_json::from_slice(&artifact.bytes).ok()?;
            for name in value.get("shards")?.as_array()? {
                shards.insert(name.as_str()?.to_owned());
            }
        }
    }
    Some(shards)
}

fn run_quality_request(
    request: &QualityRequest,
    env: Env<'_>,
    fs: &dyn FileSystem,
    kind: &mut PhaseKind,
    effects: &mut PhaseEffects,
) -> i32 {
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
        audit_today: _,
        actions,
        proc_root: _,
        bump_commit_json: _,
    } = env;
    let planned_reports = match plan_reports(
        workspace,
        request.command,
        &request.common.reports,
        &request.common.output,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(request.common.output, out, err, &error.to_string()),
    };
    if request.command == Command::Lint && !request.applies() {
        if let Some(exit) = guard_build_output(workspace, request.common.output, out, err) {
            return exit;
        }
    }
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
    ) {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(request.common.output, out, err, &error),
    };
    let mut build = match plan_build_with(
        request.command,
        &resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
        None,
    )
    .map_err(|error| format!("{error}"))
    {
        Ok(build) => build,
        Err(message) => return pre_exec(request.common.output, out, err, &message),
    };
    *kind = PhaseKind::Tool;
    let apply = request.applies();
    let mode = if apply { "default" } else { "check" };
    if request.common.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(request.command.name(), mode)) {
            return exit;
        }
    } else if request.chatty() && !stdout_report {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", build.summary)) {
            return exit;
        }
    }
    // The probe runs after the start event, so a run whose stdout already failed starts no Bazel.
    let mut probe_counts = None;
    match probe_shards(
        request,
        &resolved,
        workspace,
        runner,
        temp_dir,
        pid,
        nonce,
        actions,
        &mut probe_counts,
    ) {
        Ok(Some(shards)) => {
            if let Ok(narrowed) = plan_build_with(
                request.command,
                &resolved,
                &request.bazel_options,
                bep_text,
                &request.bazel_startup_options,
                Some(&shards),
            ) {
                build = narrowed;
            }
        }
        Ok(None) => {}
        Err(ProbeStop::Signalled) => {
            *kind = PhaseKind::Host;
            return operational_output(
                request.common.output,
                out,
                err,
                CODE_BAZEL_SIGNALLED,
                "Bazel terminated by signal",
            );
        }
        Err(ProbeStop::Interrupted) => {
            *kind = PhaseKind::Cancelled;
            return operational_output(
                request.common.output,
                out,
                err,
                CODE_BAZEL_FAILED,
                "the shard probe was interrupted",
            );
        }
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
        Err(exit) => {
            let _ = std::fs::remove_file(&bep);
            *kind = PhaseKind::Host;
            return exit;
        }
    };
    let action_counts = match (probe_counts, read_actions(&bep, actions)) {
        (Some(probe), Some(build)) => Some(probe + build),
        (probe, build) => probe.or(build),
    };
    let (compile_only, build_stage) = if bazel_code == 0 {
        (false, false)
    } else {
        match report_build_failures(&bep, request.common.output, out, err) {
            Ok(reported) => (reported.compile_only, reported.build_stage),
            Err(exit) => {
                let _ = std::fs::remove_file(&bep);
                return exit;
            }
        }
    };
    let mut collected = match collect_results(&bep, workspace) {
        Ok(collected) => collected,
        Err((code, message)) => {
            let _ = std::fs::remove_file(&bep);
            *kind = PhaseKind::Incomplete;
            return operational_output(request.common.output, out, err, &code, &message);
        }
    };
    let _ = std::fs::remove_file(&bep);
    if bazel_code != 0 {
        let crashed = bazel_code == crate::bazel_restart::RESTART_EXIT;
        let (code, failed, fix) = if crashed {
            (
                CODE_SERVER_CRASHED,
                "run",
                "run the command again after the Bazel server crash",
            )
        } else if compile_only {
            (CODE_BAZEL_FAILED, "build", "fix the compile errors above")
        } else if build_stage {
            (CODE_BAZEL_FAILED, "build", "fix the build errors above")
        } else {
            (
                CODE_ANALYSIS_FAILED,
                "analysis",
                "fix the Bazel error above",
            )
        };
        let detail = format!(
            "{} {failed} failed with Bazel exit {bazel_code}; results are incomplete",
            request.command.name()
        );
        if request.common.output == OutputMode::Json {
            if let Err(exit) =
                emit_built(out, bazel_exit_event(code, &detail, bazel_code, "execute"))
            {
                return exit;
            }
        } else {
            let _ = writeln!(err, "dx: {code}: {detail}");
            let _ = writeln!(err, "{}", rerun_hint(request, fix));
        }
    }
    collected.complete = collected.complete && bazel_code == 0;
    dx_output::sort_diagnostics(&mut collected.initial);
    dx_output::sort_diagnostics(&mut collected.terminal);
    collected
        .changes
        .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    let applied_outcome =
        apply_collected_changes(fs, workspace, apply, collected.complete, &collected.changes);
    let sources = applied_outcome.sources;
    let mut applied = applied_outcome.applied;
    let mut not_applied = applied_outcome.not_applied;
    if apply {
        for conflict in &collected.conflicts {
            applied.insert(conflict.path.clone(), false);
            not_applied.push((conflict.path.clone(), conflict.reason));
        }
    }
    *effects = PhaseEffects {
        changes: collected.changes.len() as u64,
        applied: applied.values().filter(|written| **written).count() as u64,
        not_applied: not_applied.len() as u64,
        baseline_refreshed: false,
    };
    let on_disk = terminal_on_disk(
        &applied,
        &collected.original_digests,
        &collected.terminal_digests,
    );
    let (status, _) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &on_disk,
        request.fail_on,
        !collected.changes.is_empty(),
    );
    let baseline: Option<BaselineView> = match apply_baseline(
        workspace,
        &status,
        &analyzed_scope(&collected),
        &collected.original_digests,
        BaselineGate {
            complete: collected.complete,
            stable: collected.unstable.is_empty(),
            applied_all: not_applied.is_empty(),
        },
        apply,
        request.baseline.as_deref(),
    ) {
        Ok(view) => view,
        Err((code, message)) => {
            return operational_output(request.common.output, out, err, &code, &message);
        }
    };
    effects.baseline_refreshed = baseline.as_ref().is_some_and(|view| view.refreshed);
    let suppressed: Vec<bool> = baseline
        .as_ref()
        .map_or_else(|| vec![false; status.len()], |view| view.suppressed.clone());
    let mut failed =
        status.iter().zip(&suppressed).any(|(diagnostic, held)| {
            !held && meets_threshold(diagnostic.severity, request.fail_on)
        }) || baseline.as_ref().is_some_and(|view| !view.stale.is_empty());
    if (!apply && !collected.changes.is_empty())
        || !not_applied.is_empty()
        || !collected.conflicts.is_empty()
        || !collected.unstable.is_empty()
    {
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

    let snapshots: Snapshots = if matches!(
        request.common.output,
        OutputMode::Json | OutputMode::Text { .. }
    ) {
        verified_snapshots(workspace, &collected, &sources, &status)
    } else {
        Snapshots::new()
    };
    let emit_counts = match emit_findings(
        EmitInputs {
            request,
            status: &status,
            changes: &collected.changes,
            applied: &applied,
            not_applied: &not_applied,
            conflicts: &collected.conflicts,
            patch: &patch,
            stdout_report,
            suppressed: &suppressed,
            baseline: baseline.as_ref(),
            snapshots: &snapshots,
            original_digests: &collected.original_digests,
            terminal_digests: &collected.terminal_digests,
        },
        out,
        err,
    ) {
        Ok(counts) => counts,
        Err(exit) => return exit,
    };
    let applied_count = emit_counts.applied_count;
    let not_applied_count = emit_counts.not_applied_count;
    let change_count = collected.changes.len() as u64;

    if !apply
        && !collected.changes.is_empty()
        && matches!(request.common.output, OutputMode::Text { .. })
    {
        let _ = writeln!(err, "{}", apply_hint(request, collected.changes.len()));
    }
    if request.common.output != OutputMode::Json
        && not_applied
            .iter()
            .any(|(_, reason)| *reason != REASON_INCOMPLETE_COLLECTION)
    {
        let action = "resolve the Not applied files listed above";
        let _ = writeln!(err, "{}", rerun_hint(request, action));
    }
    let reports_ok = match write_standard_reports(
        StandardReports {
            workspace,
            collected: &collected,
            sources: &sources,
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
    ) {
        Ok(ok) => ok,
        Err(exit) => return exit,
    };

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
    for item in &collected.unstable {
        let detail = item.describe();
        if request.common.output == OutputMode::Json {
            if let Err(exit) = emit_built(
                out,
                dx_output::error_event(CODE_NOT_CONVERGED, &detail, None, None, None),
            ) {
                return exit;
            }
        } else {
            let _ = writeln!(err, "dx: {CODE_NOT_CONVERGED}: {detail}");
            let _ = writeln!(
                err,
                "{}",
                rerun_hint(request, "fix the conflicting tool configuration")
            );
        }
    }
    if let Some(detail) = collected.problem_detail() {
        if request.common.output == OutputMode::Json {
            if let Err(exit) = emit_built(
                out,
                dx_output::error_event("incomplete_results", &detail, None, None, None),
            ) {
                return exit;
            }
        } else {
            let _ = writeln!(err, "dx: incomplete_results: {detail}");
            if bazel_code == 0 {
                let action = "make sure every target reports results";
                let _ = writeln!(err, "{}", rerun_hint(request, action));
            }
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
                run_id: None,
                diagnostics: Some([info, warning, error]),
                changes: Some([0, change_count]),
                effects: if apply {
                    Some([applied_count, not_applied_count])
                } else {
                    None
                },
                baseline: baseline.as_ref().map(BaselineView::counts),
                tests: None,
                actions: action_counts,
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
    }
    *kind = if !collected.complete {
        if bazel_code == crate::bazel_restart::RESTART_EXIT {
            PhaseKind::Host
        } else if compile_only {
            PhaseKind::Compile
        } else if bazel_code != 0 {
            PhaseKind::Tool
        } else {
            PhaseKind::Incomplete
        }
    } else if !collected.unstable.is_empty() {
        PhaseKind::NonConvergence
    } else if !reports_ok {
        PhaseKind::Report
    } else if failed {
        PhaseKind::Findings
    } else {
        PhaseKind::Clean
    };
    if collected.complete && !failed && reports_ok {
        0
    } else {
        1
    }
}

#[cfg(test)]
#[path = "quality_convergence.rs"]
mod quality_convergence;
#[cfg(test)]
#[path = "quality_delivery_tests.rs"]
mod quality_delivery_tests;
#[cfg(test)]
#[path = "quality_edits.rs"]
mod quality_edits;
#[cfg(test)]
#[path = "quality_json.rs"]
mod quality_json;
#[cfg(test)]
#[path = "quality_modes.rs"]
mod quality_modes;
#[cfg(test)]
#[path = "quality_probe_tests.rs"]
mod quality_probe_tests;
#[cfg(test)]
#[path = "quality_reporting.rs"]
mod quality_reporting;
#[cfg(test)]
#[path = "quality_shared.rs"]
mod quality_shared;
#[cfg(test)]
#[path = "text_output_tests.rs"]
mod text_output_tests;
