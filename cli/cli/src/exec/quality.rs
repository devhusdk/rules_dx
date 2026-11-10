use super::bazel_hint::{recorded, refine};
use super::breaking_base;
use super::build_failures::report_build_failures;
use super::common::*;
use super::guidance::{apply_hint, rerun_hint};
use super::notice_codes::{CODE_CHANGES_PROPOSED, CODE_TOOL_SKIPPED};
use super::quality_apply::{apply_collected_changes, project_status, terminal_on_disk};
use super::quality_emit::{EmitInputs, emit_findings};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{
    Snapshots, StandardReports, verified_snapshots, write_standard_reports,
};
use super::results::{collect_results, invert_repo_mapping};
use dx_apply::{FileSystem, RealFileSystem};
use dx_output::{
    ActionCounts, FindingCounts, FinishedCounts, NoticeEvent, OutputMode, Severity,
    command_finished, meets_threshold, notice_event, quality_started,
};
use dx_plan::args::{BREAKING_AGAINST_OPTION, Command, Invocation, QualityRequest};
use dx_plan::plan::{
    Consumer, SHARD_OUTPUT_GROUP, SKIP_OUTPUT_GROUP, Selection, bep_path, plan_build_with,
    plan_probe,
};
use dx_plan::reports::{Destination, plan_reports};
use dx_plan::resolve::{path_filter, resolve};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

pub(crate) fn execute_quality(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.quality_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { out, err, .. } = env;
            return pre_exec(invocation.output, out, err, message.message());
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
        edit_commit_json,
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
            edit_commit_json,
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
            if output != OutputMode::Json
                && let Some(note) = report.committed_skipped
            {
                let _ = writeln!(err, "dx: build-output check: {note}");
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
    consumer: Option<Consumer>,
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
        consumer,
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

/// The skip notes of every target in the event file, one per tool however many targets dropped it.
pub(super) fn read_skip_notes(bep: &Path, workspace: &Path) -> Vec<String> {
    let Ok(account) = collect_output_account(bep, SKIP_OUTPUT_GROUP, workspace) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut notes = Vec::new();
    for target in account.targets.iter().chain(&account.no_work) {
        for artifact in &target.artifacts {
            for line in String::from_utf8_lossy(&artifact.bytes).lines() {
                let tool = line.split(':').next().unwrap_or(line);
                if !line.is_empty() && seen.insert(tool.to_owned()) {
                    notes.push(line.to_owned());
                }
            }
        }
    }
    notes.sort();
    notes
}

/// The warning for a skip note of the form `<tool>: <reason>`.
pub(super) fn skip_notice(command: &str, note: &str) -> NoticeEvent {
    NoticeEvent {
        related_command: Some(command.to_owned()),
        ..NoticeEvent::warning(CODE_TOOL_SKIPPED, note)
    }
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
        audit_today,
        actions,
        proc_root: _,
        edit_commit_json: _,
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
    if request.command == Command::Lint
        && !request.applies()
        && let Some(exit) = guard_build_output(workspace, request.common.output, out, err)
    {
        return exit;
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
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let rewritten;
    let request = if request
        .bazel_options
        .iter()
        .any(|option| option.starts_with(BREAKING_AGAINST_OPTION))
    {
        let context = breaking_base::Context {
            workspace,
            runner,
            query_runner,
            startup: &request.bazel_startup_options,
            temp_dir,
        };
        match breaking_base::resolve_options(&request.bazel_options, &context) {
            Ok(bazel_options) => {
                rewritten = QualityRequest {
                    bazel_options,
                    ..request.clone()
                };
                &rewritten
            }
            Err(error) => {
                return report_dx_error(
                    request.common.output,
                    out,
                    err,
                    CODE_INVALID_ARGUMENTS,
                    &error,
                );
            }
        }
    } else {
        request
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
    // An unreadable MODULE.bazel gives no deps, and the rules_dx root carries every family;
    // in both cases every family's aspects stay.
    let deps: Vec<String> = dx_adopt::module_deps(workspace).into_iter().collect();
    let installed = (!deps.is_empty() && !dx_adopt::is_rules_dx_root(workspace)).then_some(deps);
    let extensions = installed
        .as_ref()
        .map(|_| dx_adopt::file_extensions(workspace));
    let consumer = installed
        .as_deref()
        .zip(extensions.as_ref())
        .map(|(modules, extensions)| Consumer {
            modules,
            extensions,
        });
    let mut build = match plan_build_with(
        request.command,
        &resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
        Selection {
            shards: None,
            consumer,
            unsafe_fixes: request.unsafe_fixes,
        },
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
        if let Err(exit) = emit_built(
            out,
            quality_started(request.command.name(), mode, request.unsafe_fixes),
        ) {
            return exit;
        }
    } else if request.chatty()
        && !stdout_report
        && let Err(exit) = check_stdout_write(writeln!(out, "{}", build.summary))
    {
        return exit;
    }
    if let Err(exit) = warn_scope_excluded(request.common.output, out, err, &resolved.kept) {
        return exit;
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
        consumer,
    ) {
        Ok(Some(shards)) => {
            if let Ok(narrowed) = plan_build_with(
                request.command,
                &resolved,
                &request.bazel_options,
                bep_text,
                &request.bazel_startup_options,
                Selection {
                    shards: Some(&shards),
                    consumer,
                    unsafe_fixes: request.unsafe_fixes,
                },
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
    #[allow(clippy::disallowed_methods)] // dump_repo_mapping takes no build settings
    let repo_mapping = || {
        let mut argv = dx_process::startup_argv(&request.bazel_startup_options);
        argv.extend(["mod", "dump_repo_mapping", ""].map(str::to_owned));
        let result = query_runner.run_query(&argv, workspace).ok()?;
        (result.code == Some(0))
            .then(|| invert_repo_mapping(&result.stdout))
            .flatten()
    };
    let mut collected = match collect_results(&bep, workspace, &repo_mapping) {
        Ok(collected) => collected,
        Err(error) => {
            let _ = std::fs::remove_file(&bep);
            *kind = PhaseKind::Incomplete;
            return report_dx_error(
                request.common.output,
                out,
                err,
                bep_error_code(&error),
                &error,
            );
        }
    };
    for note in read_skip_notes(&bep, workspace) {
        if request.common.output == OutputMode::Json {
            let notice = skip_notice(request.command.name(), &note);
            if let Err(exit) = emit_built(out, notice_event(&notice)) {
                let _ = std::fs::remove_file(&bep);
                return exit;
            }
        } else {
            let _ = writeln!(err, "dx: note: {note}");
        }
    }
    let _ = std::fs::remove_file(&bep);
    if bazel_code != 0 {
        let crashed = server_died(bazel_code);
        let (code, failed, fix) = if crashed {
            (
                bazel_failure_code(bazel_code),
                "run",
                "run the command again, or raise the Bazel server heap if it ran out of memory",
            )
        } else if compile_only {
            (
                refine(CODE_BAZEL_FAILED),
                "build",
                "fix the compile errors above",
            )
        } else if build_stage {
            (
                refine(CODE_BAZEL_FAILED),
                "build",
                "fix the build errors above",
            )
        } else {
            (
                refine(CODE_ANALYSIS_FAILED),
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
            match recorded().filter(|hint| hint.code == code) {
                Some(hint) => {
                    let _ = writeln!(err, "{}", hint.next);
                }
                None => {
                    let _ = writeln!(err, "{}", rerun_hint(request, fix));
                }
            }
        }
    }
    collected.complete = collected.complete && bazel_code == 0;
    if let Some(filter) = path_filter(&request.targets, workspace) {
        collected.keep_paths(&filter);
    }
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
    };
    let on_disk = terminal_on_disk(
        &applied,
        &collected.original_digests,
        &collected.terminal_digests,
    );
    let (mut status, _) = project_status(
        apply,
        &collected.initial,
        &collected.terminal,
        &on_disk,
        request.fail_on,
        !collected.changes.is_empty(),
    );
    super::quality_security::drop_excepted_findings(workspace, audit_today, &mut status);
    if request.command == Command::Lint {
        status.extend(super::generated_drift::findings(
            workspace,
            query_runner,
            &super::preset_sync::rewritten(),
        ));
    }
    let mut failed = status
        .iter()
        .any(|diagnostic| meets_threshold(diagnostic.severity, request.fail_on));
    if (!apply && !collected.changes.is_empty())
        || !not_applied.is_empty()
        || !collected.conflicts.is_empty()
        || !collected.unstable.is_empty()
    {
        failed = true;
    }

    let mut patch = String::new();
    if request.common.output == OutputMode::Diff {
        let verified: Vec<FileChange> = collected
            .changes
            .iter()
            .filter(|change| {
                !apply || matches!(sources.get(&change.path), Some(SourceRead::Bytes(_)))
            })
            .cloned()
            .collect();
        match render_diff_patch(&sources, &verified) {
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

    if !apply && !collected.changes.is_empty() {
        if matches!(request.common.output, OutputMode::Text { .. }) {
            let _ = writeln!(err, "{}", apply_hint(request, collected.changes.len()));
        } else if request.common.output == OutputMode::Json {
            let notice = NoticeEvent {
                related_command: Some(request.command.name().to_owned()),
                ..NoticeEvent::info(
                    CODE_CHANGES_PROPOSED,
                    apply_hint(request, collected.changes.len()),
                )
            };
            if let Err(exit) = emit_built(out, notice_event(&notice)) {
                return exit;
            }
        }
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
    let mut finding_counts = FindingCounts {
        omitted: emit_counts.findings_omitted,
        ..FindingCounts::default()
    };
    for diagnostic in &status {
        *finding_counts
            .by_tool
            .entry(diagnostic.tool.clone())
            .or_insert(0) += 1;
        if let Some(rule) = &diagnostic.rule {
            *finding_counts.by_rule.entry(rule.clone()).or_insert(0) += 1;
        }
        match diagnostic.severity {
            Severity::Info => info += 1,
            Severity::Warning => warning += 1,
            Severity::Error => error += 1,
        }
    }
    if matches!(request.common.output, OutputMode::Text { .. }) && collected.complete && !failed {
        let _ = writeln!(err, "{}", findings_summary(&status, resolved.kept.len()));
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
    let exit_code = if collected.complete && !failed && reports_ok {
        0
    } else if bazel_code != 0 {
        bazel_code
    } else {
        1
    };
    if request.common.output == OutputMode::Json {
        let finished = command_finished(
            exit_code,
            &FinishedCounts {
                results_complete: Some(collected.complete),
                run_id: None,
                diagnostics: Some([info, warning, error]),
                findings: Some(finding_counts),
                changes: Some([0, change_count]),
                effects: if apply {
                    Some([applied_count, not_applied_count])
                } else {
                    None
                },
                tests: None,
                actions: action_counts,
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
    }
    *kind = if !collected.complete {
        if server_died(bazel_code) {
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
    exit_code
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
#[path = "quality_skip_notes_tests.rs"]
mod quality_skip_notes_tests;
#[cfg(test)]
#[path = "text_output_tests.rs"]
mod text_output_tests;
