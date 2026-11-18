use super::breaking_base;
use super::build_failures::report_build_failures;
use super::check_build::{SharedChecks, SharedRead};
use super::link_check;
use super::pinned_tool::QueryBazel;
use super::quality_apply::{apply_collected_changes, project_status, terminal_on_disk};
use super::quality_emit::{EmitCounts, EmitInputs, emit_findings};
use super::quality_patch::render_diff_patch;
use super::quality_reports::{
    Snapshots, StandardReports, verified_snapshots, write_standard_reports,
};
use super::results::{Collected, collect_results, invert_repo_mapping};
use dx_apply::{FileSystem, RealFileSystem};
use dx_exec_core::bazel_hint::{recorded, refine};
use dx_exec_core::common::{
    ActionTally, BAZEL_INTERRUPTED, CODE_ANALYSIS_FAILED, CODE_BAZEL_FAILED, CODE_BAZEL_SIGNALLED,
    CODE_DIFF_FAILED, CODE_INVALID_ARGUMENTS, CODE_NOT_CONVERGED, CODE_UNREADABLE_BEP, Env,
    Failure, FileChange, LatchedOut, PhaseEffects, PhaseKind, PhaseRun,
    REASON_INCOMPLETE_COLLECTION, SourceRead, bazel_exit_event, bazel_failure_code, bep_error_code,
    check_stdout_write, collect_output_account, collect_own_account, emit_built, emit_event,
    findings_summary, operational_output, pre_exec, read_actions, report_dx_error, report_failure,
    resolve_failed, run_bazel_output, server_died, warn_scope_excluded,
};
use dx_exec_core::guidance::{apply_hint, rerun_hint};
use dx_exec_core::notice_codes::{CODE_CHANGES_PROPOSED, CODE_TOOL_SKIPPED};
use dx_output::{
    ActionCounts, DiagnosticEvent, FindingCounts, FinishedCounts, NoticeEvent, OutputMode,
    Severity, command_finished, meets_threshold, notice_event, quality_started,
};
use dx_plan::args::{BREAKING_AGAINST_OPTION, Command, Invocation, QualityRequest};
use dx_plan::plan::{
    BuildPlan, Consumer, SHARD_OUTPUT_GROUP, SKIP_OUTPUT_GROUP, Selection, bep_path,
    plan_build_with, plan_probe,
};
use dx_plan::reports::{Destination, PlannedReport, plan_reports};
use dx_plan::resolve::{ResolvedScope, path_filter, resolve};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

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
        None,
    )
}

/// Runs one umbrella quality phase, from the build it shares with the other phases when given.
pub(crate) fn execute_quality_phase(
    request: &QualityRequest,
    env: Env<'_>,
    shared: Option<&mut SharedChecks>,
) -> PhaseRun {
    execute_quality_phase_with_fs(request, env, &RealFileSystem, shared)
}

/// Runs one quality phase whose apply writes go through `fs`.
pub(crate) fn execute_quality_phase_with_fs(
    request: &QualityRequest,
    env: Env<'_>,
    fs: &dyn FileSystem,
    shared: Option<&mut SharedChecks>,
) -> PhaseRun {
    let mut kind = PhaseKind::Selection;
    let mut effects = PhaseEffects::default();
    let code = run_quality(request, env, fs, &mut kind, &mut effects, shared);
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
    shared: Option<&mut SharedChecks>,
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
        shared,
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
                writeln!(err, "dx: build-output check: {note}").ok();
            }
            None
        }
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
/// before its event file is removed.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct input"
)]
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
    consumer: Option<Consumer<'_>>,
) -> Result<Option<BTreeSet<String>>, ProbeStop> {
    let bep = temp_dir.join(format!("dx-bep-{pid}-{nonce}-probe.json"));
    let Some(bep_text) = bep.to_str() else {
        return Ok(None);
    };
    let Ok(Some(plan)) = plan_probe(
        request.command,
        resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
        consumer,
    ) else {
        return Ok(None);
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
    std::fs::remove_file(&bep).ok();
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
pub(super) fn read_skip_notes(
    bep: &Path,
    foreign: &BTreeSet<String>,
    workspace: &Path,
) -> Vec<String> {
    let Ok(account) = collect_own_account(bep, SKIP_OUTPUT_GROUP, workspace, foreign) else {
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

struct Settled {
    sources: BTreeMap<String, SourceRead>,
    applied: BTreeMap<String, bool>,
    not_applied: Vec<(String, &'static str)>,
    status: Vec<DiagnosticEvent>,
    failed: bool,
}

/// The build events a phase reads, and whether the phase owns them and removes them after.
struct BepRead {
    path: PathBuf,
    /// The aspects of other phases sharing the build, whose outputs this phase leaves out.
    foreign: BTreeSet<String>,
    owned: bool,
}

impl BepRead {
    fn discard(&self) {
        if self.owned {
            std::fs::remove_file(&self.path).ok();
        }
    }
}

fn run_quality_request(
    request: &QualityRequest,
    mut env: Env<'_>,
    fs: &dyn FileSystem,
    kind: &mut PhaseKind,
    effects: &mut PhaseEffects,
    mut shared: Option<&mut SharedChecks>,
) -> i32 {
    let planned_reports = match plan_quality_reports(request, &mut env) {
        Ok(planned) => planned,
        Err(exit) => return exit,
    };
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let bep = bep_path(env.temp_dir, env.pid, env.nonce);
    let Some(bep_text) = bep.to_str() else {
        return operational_output(
            request.common.output,
            env.out,
            env.err,
            CODE_UNREADABLE_BEP,
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let rewritten = match rewrite_breaking_base(request, &mut env) {
        Ok(rewritten) => rewritten,
        Err(exit) => return exit,
    };
    let raw = request;
    let request = rewritten.as_ref().unwrap_or(request);
    let resolved = match resolve(
        &request.targets,
        env.workspace,
        env.query_runner,
        &request.bazel_startup_options,
    ) {
        Ok(resolved) => resolved,
        Err(error) => {
            return resolve_failed(request.common.output, env.out, env.err, &error);
        }
    };
    // An unreadable MODULE.bazel gives no deps, and the rules_dx root carries every family.
    let deps: Vec<String> = dx_adopt::module_deps(env.workspace).into_iter().collect();
    let extensions = (!deps.is_empty() && !dx_adopt::is_rules_dx_root(env.workspace))
        .then(|| dx_adopt::file_extensions(env.workspace));
    let consumer = extensions
        .as_ref()
        .map(|extensions| Consumer { extensions });
    let planned = plan_quality_build(
        request,
        &mut env,
        kind,
        (&resolved, consumer),
        bep_text,
        stdout_report,
        shared.is_none(),
    );
    let (build, probe_counts) = match planned {
        Ok(planned) => planned,
        Err(exit) => return exit,
    };
    let shared_read = match shared.as_deref_mut() {
        Some(shared) => match shared.read(raw, request, &resolved, consumer, &mut env, kind) {
            Ok(read) => read,
            Err(exit) => return exit,
        },
        None => None,
    };
    let (build_run, events) = match shared_read {
        Some(read) => match shared_build_run(request, &mut env, &read) {
            Ok(run) => (
                run,
                BepRead {
                    path: read.bep,
                    foreign: read.foreign,
                    owned: false,
                },
            ),
            Err(exit) => return exit,
        },
        None => match run_quality_build(request, &mut env, kind, &build, &bep, probe_counts) {
            Ok(run) => (
                run,
                BepRead {
                    path: bep.clone(),
                    foreign: BTreeSet::new(),
                    owned: true,
                },
            ),
            Err(exit) => return exit,
        },
    };
    let bazel_code = build_run.code;
    let compile_only = build_run.compile_only;
    let action_counts = build_run.actions;
    let collected = collect_run(request, &mut env, kind, &events);
    if let Some(shared) = shared {
        shared.finish_read();
    }
    let mut collected = match collected {
        Ok(collected) => collected,
        Err(exit) => return exit,
    };
    if let Err(exit) = report_bazel_failure(request, &mut env, &build_run) {
        return exit;
    }
    collected.complete = collected.complete && bazel_code == 0;
    let apply = request.applies();
    let settled = settle_changes(request, &mut env, fs, &mut collected, effects);
    let (emit_counts, reports_ok) = match emit_results(
        request,
        &mut env,
        &collected,
        &settled,
        (&planned_reports, stdout_report),
    ) {
        Ok(emitted) => emitted,
        Err(exit) => return exit,
    };
    let Settled { status, failed, .. } = &settled;
    let (diagnostics, finding_counts) = tally_findings(status, emit_counts.findings_omitted);
    if matches!(request.common.output, OutputMode::Text { .. }) && collected.complete && !failed {
        writeln!(env.err, "{}", findings_summary(status, resolved.kept.len())).ok();
    }
    if let Err(exit) = report_incomplete(request, &mut env, &collected, bazel_code) {
        return exit;
    }
    let exit_code = if collected.complete && !failed && reports_ok {
        0
    } else if bazel_code != 0 {
        bazel_code
    } else {
        1
    };
    if request.common.output == OutputMode::Json {
        let counts = FinishedCounts {
            results_complete: Some(collected.complete),
            run_id: None,
            diagnostics: Some(diagnostics),
            findings: Some(finding_counts),
            changes: Some([0, collected.changes.len() as u64]),
            effects: apply.then_some([emit_counts.applied_count, emit_counts.not_applied_count]),
            tests: None,
            actions: action_counts,
        };
        if let Err(exit) = emit_event(env.out, &command_finished(exit_code, &counts)) {
            return exit;
        }
    }
    *kind = quality_phase(&collected, bazel_code, compile_only, reports_ok, *failed);
    exit_code
}

struct BuildRun {
    code: i32,
    compile_only: bool,
    build_stage: bool,
    actions: Option<ActionCounts>,
}

fn run_quality_build(
    request: &QualityRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    build: &BuildPlan,
    bep: &Path,
    probe_counts: Option<ActionCounts>,
) -> Result<BuildRun, i32> {
    let bazel_code = match run_bazel_output(
        request.common.output,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        &build.argv,
        &[],
    ) {
        Ok(code) => code,
        Err(exit) => {
            std::fs::remove_file(bep).ok();
            *kind = PhaseKind::Host;
            return Err(exit);
        }
    };
    let action_counts = match (probe_counts, read_actions(bep, env.actions)) {
        (Some(probe), Some(build)) => Some(probe + build),
        (probe, build) => probe.or(build),
    };
    let (compile_only, build_stage) = if bazel_code == 0 {
        (false, false)
    } else {
        match report_build_failures(bep, request.common.output, env.out, env.err) {
            Ok(reported) => (reported.compile_only, reported.build_stage),
            Err(exit) => {
                std::fs::remove_file(bep).ok();
                return Err(exit);
            }
        }
    };
    Ok(BuildRun {
        code: bazel_code,
        compile_only,
        build_stage,
        actions: action_counts,
    })
}

/// The build outcome of one phase that read a shared build.
///
/// Bazel's failures name targets but not aspects, so a phase that owns a failure reports every
/// failure of the shared build, as its own build would have for the targets it shares.
fn shared_build_run(
    request: &QualityRequest,
    env: &mut Env<'_>,
    read: &SharedRead,
) -> Result<BuildRun, i32> {
    let (compile_only, build_stage) = if read.code == 0 {
        (false, false)
    } else {
        let reported = report_build_failures(&read.bep, request.common.output, env.out, env.err)?;
        (reported.compile_only, reported.build_stage)
    };
    Ok(BuildRun {
        code: read.code,
        compile_only,
        build_stage,
        actions: read.actions,
    })
}

fn plan_quality_reports(
    request: &QualityRequest,
    env: &mut Env<'_>,
) -> Result<Vec<PlannedReport>, i32> {
    let planned_reports = plan_reports(
        env.workspace,
        request.command,
        &request.common.reports,
        &request.common.output,
    )
    .map_err(|error| pre_exec(request.common.output, env.out, env.err, &error.to_string()))?;
    if request.command == Command::Lint
        && !request.applies()
        && let Some(exit) =
            guard_build_output(env.workspace, request.common.output, env.out, env.err)
    {
        return Err(exit);
    }
    Ok(planned_reports)
}

/// The request with a `--breaking-against` ref resolved to a base, or `None` when it has none.
fn rewrite_breaking_base(
    request: &QualityRequest,
    env: &mut Env<'_>,
) -> Result<Option<QualityRequest>, i32> {
    if !request
        .bazel_options
        .iter()
        .any(|option| option.starts_with(BREAKING_AGAINST_OPTION))
    {
        return Ok(None);
    }
    let context = breaking_base::Context {
        workspace: env.workspace,
        runner: env.runner,
        query_runner: env.query_runner,
        startup: &request.bazel_startup_options,
        temp_dir: env.temp_dir,
    };
    match breaking_base::resolve_options(&request.bazel_options, &context) {
        Ok(bazel_options) => Ok(Some(QualityRequest {
            bazel_options,
            ..request.clone()
        })),
        Err(error) => Err(report_dx_error(
            request.common.output,
            env.out,
            env.err,
            CODE_INVALID_ARGUMENTS,
            &error,
        )),
    }
}

fn announce_quality_start(
    request: &QualityRequest,
    env: &mut Env<'_>,
    resolved: &ResolvedScope,
    summary: &str,
    stdout_report: bool,
) -> Result<(), i32> {
    let out = &mut *env.out;
    let err = &mut *env.err;
    let apply = request.applies();
    let mode = if apply { "default" } else { "check" };
    if request.common.output == OutputMode::Json {
        emit_built(
            out,
            quality_started(request.command.name(), mode, request.unsafe_fixes),
        )?;
    } else if request.chatty()
        && !stdout_report
        && let Err(exit) = check_stdout_write(writeln!(out, "{summary}"))
    {
        return Err(exit);
    }
    warn_scope_excluded(request.common.output, out, err, &resolved.kept)?;
    Ok(())
}

/// Plans the phase's own build and announces the phase; `probe` narrows the build first.
fn plan_quality_build(
    request: &QualityRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    (resolved, consumer): (&ResolvedScope, Option<Consumer<'_>>),
    bep_text: &str,
    stdout_report: bool,
    probe: bool,
) -> Result<(BuildPlan, Option<ActionCounts>), i32> {
    let (workspace, runner, temp_dir, pid, nonce, actions) = (
        env.workspace,
        env.runner,
        env.temp_dir,
        env.pid,
        env.nonce,
        env.actions,
    );
    let mut build = match plan_build_with(
        request.command,
        resolved,
        &request.bazel_options,
        bep_text,
        &request.bazel_startup_options,
        Selection {
            shards: None,
            consumer,
            unsafe_fixes: request.unsafe_fixes,
            online: request.online,
        },
    )
    .map_err(|error| format!("{error}"))
    {
        Ok(build) => build,
        Err(message) => return Err(pre_exec(request.common.output, env.out, env.err, &message)),
    };
    *kind = PhaseKind::Tool;
    announce_quality_start(request, env, resolved, &build.summary, stdout_report)?;
    if !probe {
        return Ok((build, None));
    }
    // The probe runs after the start event, so a run whose stdout already failed starts no Bazel.
    let mut probe_counts = None;
    match probe_shards(
        request,
        resolved,
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
                resolved,
                &request.bazel_options,
                bep_text,
                &request.bazel_startup_options,
                Selection {
                    shards: Some(&shards),
                    consumer,
                    unsafe_fixes: request.unsafe_fixes,
                    online: request.online,
                },
            ) {
                build = narrowed;
            }
        }
        Ok(None) => {}
        Err(ProbeStop::Signalled) => {
            *kind = PhaseKind::Host;
            return Err(operational_output(
                request.common.output,
                env.out,
                env.err,
                CODE_BAZEL_SIGNALLED,
                "Bazel terminated by signal",
            ));
        }
        Err(ProbeStop::Interrupted) => {
            *kind = PhaseKind::Cancelled;
            return Err(operational_output(
                request.common.output,
                env.out,
                env.err,
                CODE_BAZEL_FAILED,
                "the shard probe was interrupted",
            ));
        }
    }
    Ok((build, probe_counts))
}

fn collect_run(
    request: &QualityRequest,
    env: &mut Env<'_>,
    kind: &mut PhaseKind,
    events: &BepRead,
) -> Result<Collected, i32> {
    let (bep, foreign) = (events.path.as_path(), &events.foreign);
    let (workspace, query_runner, temp_dir, pid, nonce) = (
        env.workspace,
        env.query_runner,
        env.temp_dir,
        env.pid,
        env.nonce,
    );
    let out = &mut *env.out;
    let err = &mut *env.err;
    #[expect(
        clippy::disallowed_methods,
        reason = "dump_repo_mapping takes no build settings"
    )]
    let repo_mapping = || {
        let mut argv = dx_process::startup_argv(&request.bazel_startup_options);
        argv.extend(["mod", "dump_repo_mapping", ""].map(str::to_owned));
        let result = query_runner.run_query(&argv, workspace).ok()?;
        (result.code == Some(0))
            .then(|| invert_repo_mapping(&result.stdout))
            .flatten()
    };
    let mut collected = match collect_results(bep, foreign, workspace, &repo_mapping) {
        Ok(collected) => collected,
        Err(error) => {
            events.discard();
            *kind = PhaseKind::Incomplete;
            return Err(report_dx_error(
                request.common.output,
                out,
                err,
                bep_error_code(&error),
                &error,
            ));
        }
    };
    for note in read_skip_notes(bep, foreign, workspace) {
        if request.common.output == OutputMode::Json {
            let notice = skip_notice(request.command.name(), &note);
            if let Err(exit) = emit_built(out, notice_event(&notice)) {
                events.discard();
                return Err(exit);
            }
        } else {
            writeln!(err, "dx: note: {note}").ok();
        }
    }
    let link_requests = if request.online {
        link_check::read_requests(bep, foreign, workspace)
    } else {
        Vec::new()
    };
    events.discard();
    if !link_requests.is_empty() {
        let bazel = QueryBazel {
            query_runner,
            workspace,
            startup: &request.bazel_startup_options,
        };
        match link_check::Roots::query(&bazel) {
            Ok(roots) => {
                let tag = format!("dx-link-{pid}-{nonce}");
                let outcome = link_check::run(
                    &link_requests,
                    &roots,
                    temp_dir,
                    &tag,
                    &|request, output| {
                        quality_runner::entry::run_request(request, output, Some("link-check"))
                            .map_err(|error| error.to_string())
                    },
                );
                for result in &outcome.results {
                    collected.absorb_host_result(result, "dx lint --online", workspace);
                }
                collected.problems.extend(outcome.problems);
            }
            Err(error) => collected.problems.push(format!(
                "link check could not locate Bazel's roots: {error}"
            )),
        }
    }
    Ok(collected)
}

fn report_bazel_failure(
    request: &QualityRequest,
    env: &mut Env<'_>,
    run: &BuildRun,
) -> Result<(), i32> {
    let BuildRun {
        code: bazel_code,
        compile_only,
        build_stage,
        ..
    } = *run;
    if bazel_code == 0 {
        return Ok(());
    }
    let out = &mut *env.out;
    let err = &mut *env.err;
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
        emit_built(out, bazel_exit_event(code, &detail, bazel_code, "execute"))?;
    } else {
        writeln!(err, "dx: {code}: {detail}").ok();
        match recorded().filter(|hint| hint.code == code) {
            Some(hint) => {
                writeln!(err, "{}", hint.next).ok();
            }
            None => {
                writeln!(err, "{}", rerun_hint(request, fix)).ok();
            }
        }
    }
    Ok(())
}

fn settle_changes(
    request: &QualityRequest,
    env: &mut Env<'_>,
    fs: &dyn FileSystem,
    collected: &mut Collected,
    effects: &mut PhaseEffects,
) -> Settled {
    let (workspace, query_runner, audit_today) = (env.workspace, env.query_runner, env.audit_today);
    let apply = request.applies();
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
    Settled {
        sources,
        applied,
        not_applied,
        status,
        failed,
    }
}

fn emit_results(
    request: &QualityRequest,
    env: &mut Env<'_>,
    collected: &Collected,
    settled: &Settled,
    reports: (&[PlannedReport], bool),
) -> Result<(EmitCounts, bool), i32> {
    let (planned_reports, stdout_report) = reports;
    let Settled {
        sources,
        applied,
        not_applied,
        status,
        ..
    } = settled;
    let workspace = env.workspace;
    let apply = request.applies();
    let out = &mut *env.out;
    let err = &mut *env.err;
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
        match render_diff_patch(sources, &verified) {
            Ok(rendered) => patch = rendered,
            Err(error) => {
                return Err(operational_output(
                    request.common.output,
                    out,
                    err,
                    CODE_DIFF_FAILED,
                    &error.to_string(),
                ));
            }
        }
    }

    let snapshots: Snapshots = if matches!(
        request.common.output,
        OutputMode::Json | OutputMode::Text { .. }
    ) {
        verified_snapshots(workspace, collected, sources, status)
    } else {
        Snapshots::new()
    };
    let emit_counts = emit_findings(
        EmitInputs {
            request,
            status,
            changes: &collected.changes,
            applied,
            not_applied,
            conflicts: &collected.conflicts,
            patch: &patch,
            stdout_report,
            snapshots: &snapshots,
            original_digests: &collected.original_digests,
            terminal_digests: &collected.terminal_digests,
        },
        out,
        err,
    )?;

    if !apply && !collected.changes.is_empty() {
        if matches!(request.common.output, OutputMode::Text { .. }) {
            writeln!(err, "{}", apply_hint(request, collected.changes.len())).ok();
        } else if request.common.output == OutputMode::Json {
            let notice = NoticeEvent {
                related_command: Some(request.command.name().to_owned()),
                ..NoticeEvent::info(
                    CODE_CHANGES_PROPOSED,
                    apply_hint(request, collected.changes.len()),
                )
            };
            emit_built(out, notice_event(&notice))?;
        }
    }
    if request.common.output != OutputMode::Json
        && not_applied
            .iter()
            .any(|(_, reason)| *reason != REASON_INCOMPLETE_COLLECTION)
    {
        let action = "resolve the Not applied files listed above";
        writeln!(err, "{}", rerun_hint(request, action)).ok();
    }
    let reports_ok = write_standard_reports(
        StandardReports {
            workspace,
            collected,
            sources,
            status,
            planned: planned_reports,
            output: &request.common.output,
            stdout_report,
        },
        out,
        err,
    )?;
    Ok((emit_counts, reports_ok))
}

fn tally_findings(status: &[DiagnosticEvent], omitted: u64) -> ([u64; 3], FindingCounts) {
    let mut counts = (0u64, 0u64, 0u64);
    let mut finding_counts = FindingCounts {
        omitted,
        ..FindingCounts::default()
    };
    for diagnostic in status {
        *finding_counts
            .by_tool
            .entry(diagnostic.tool.clone())
            .or_insert(0) += 1;
        if let Some(rule) = &diagnostic.rule {
            *finding_counts.by_rule.entry(rule.clone()).or_insert(0) += 1;
        }
        match diagnostic.severity {
            Severity::Info => counts.0 += 1,
            Severity::Warning => counts.1 += 1,
            Severity::Error => counts.2 += 1,
        }
    }
    ([counts.0, counts.1, counts.2], finding_counts)
}

fn report_incomplete(
    request: &QualityRequest,
    env: &mut Env<'_>,
    collected: &Collected,
    bazel_code: i32,
) -> Result<(), i32> {
    let out = &mut *env.out;
    let err = &mut *env.err;
    for item in &collected.unstable {
        let detail = item.describe();
        if request.common.output == OutputMode::Json {
            emit_built(
                out,
                dx_output::error_event(CODE_NOT_CONVERGED, &detail, None, None, None),
            )?;
        } else {
            writeln!(err, "dx: {CODE_NOT_CONVERGED}: {detail}").ok();
            writeln!(
                err,
                "{}",
                rerun_hint(request, "fix the conflicting tool configuration")
            )
            .ok();
        }
    }
    if let Some(detail) = collected.problem_detail() {
        if request.common.output == OutputMode::Json {
            emit_built(
                out,
                dx_output::error_event("incomplete_results", &detail, None, None, None),
            )?;
        } else {
            writeln!(err, "dx: incomplete_results: {detail}").ok();
            if bazel_code == 0 {
                let action = "make sure every target reports results";
                writeln!(err, "{}", rerun_hint(request, action)).ok();
            }
        }
    }
    Ok(())
}

fn quality_phase(
    collected: &Collected,
    bazel_code: i32,
    compile_only: bool,
    reports_ok: bool,
    failed: bool,
) -> PhaseKind {
    if !collected.complete {
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
#[path = "quality_skip_notes_tests.rs"]
mod quality_skip_notes_tests;
#[cfg(test)]
#[path = "text_output_tests.rs"]
mod text_output_tests;
