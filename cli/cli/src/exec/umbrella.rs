use super::common::*;
use super::failure::CODE_INVALID_SCOPE;
use super::generate::execute_generate_phase;
use super::generate::execute_generate_request;
use super::quality::execute_quality_request;
use super::quality::{execute_quality_phase, guard_build_output};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, command_started, error_event,
    nest_phase_event, notice_event, quality_started, read_event, report_event, with_correlation,
    write_event,
};
use dx_plan::args::{Command, Invocation, ReportRequest, UmbrellaRequest};
use dx_plan::plan::spec;
use dx_plan::reports::plan_reports;
use dx_plan::resolve::{MemoizedGraphQuery, QueryRunner};
use dx_process::{Runner, interrupted};
use serde_json::{Value, json};
use serde_sarif::sarif::Sarif;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const UMBRELLA_PHASES: [Command; 4] = [
    Command::Generate,
    Command::Format,
    Command::Lint,
    Command::Typecheck,
];

const RERUN_GENERATE: &str = "rerun/generate";

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";

/// Where one umbrella phase ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseState {
    Planned,
    Executed,
    Failed,
    Skipped,
}

/// What the capture one umbrella phase owed turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptureState {
    Absent,
    Unreadable,
    Malformed,
    Collected(Value),
}

/// One capture the umbrella asked a phase to write.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PhaseCapture {
    path: PathBuf,
    state: CaptureState,
}

/// One umbrella phase, how it ended and the capture it owed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PhaseOutcome {
    phase: Command,
    label: &'static str,
    state: PhaseState,
    kind: Option<PhaseKind>,
    capture: Option<PhaseCapture>,
}

/// Names why one executed phase's capture is unusable.
fn capture_reason(capture: &PhaseCapture) -> Option<&'static str> {
    match capture.state {
        CaptureState::Absent => Some("wrote no sarif capture"),
        CaptureState::Unreadable => Some("sarif capture is unreadable"),
        CaptureState::Malformed => Some("sarif capture is malformed"),
        CaptureState::Collected(_) => None,
    }
}

/// Parses one phase capture with the shared SARIF model.
fn parse_capture(bytes: &[u8]) -> Result<Value, CaptureState> {
    let document: Value = serde_json::from_slice(bytes).map_err(|_| CaptureState::Malformed)?;
    if !matches!(document.get("runs"), Some(Value::Array(_))) {
        return Err(CaptureState::Malformed);
    }
    serde_json::from_slice::<Sarif>(bytes).map_err(|_| CaptureState::Malformed)?;
    Ok(document)
}

/// The typed outcome and the artifacts of every planned umbrella phase.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct PhaseCollector {
    phases: Vec<PhaseOutcome>,
    skip_cause: String,
}

impl PhaseCollector {
    /// Records one phase as planned and waiting to run under `label`.
    fn plan(&mut self, phase: Command, label: &'static str) {
        self.phases.push(PhaseOutcome {
            phase,
            label,
            state: PhaseState::Planned,
            kind: None,
            capture: None,
        });
    }

    /// Records the capture the phase at `index` owes.
    fn require_capture(&mut self, index: usize, path: PathBuf) {
        if let Some(running) = self.phases.get_mut(index) {
            running.capture = Some(PhaseCapture {
                path,
                state: CaptureState::Absent,
            });
        }
    }

    /// Records the exit code of the phase at `index`.
    fn record_exit(&mut self, index: usize, code: i32, kind: PhaseKind) {
        if let Some(running) = self.phases.get_mut(index) {
            running.kind = Some(kind);
            running.state = if code == 0 {
                PhaseState::Executed
            } else {
                PhaseState::Failed
            };
        }
    }

    /// Records every phase that never started as skipped.
    fn skip_remaining(&mut self, cause: String) {
        self.skip_cause = cause;
        for phase in &mut self.phases {
            if phase.state == PhaseState::Planned {
                phase.state = PhaseState::Skipped;
            }
        }
    }

    /// Reads and validates every capture a phase that ran owed, failed or not.
    fn collect(&mut self) {
        for phase in &mut self.phases {
            if !matches!(phase.state, PhaseState::Executed | PhaseState::Failed) {
                continue;
            }
            let Some(capture) = phase.capture.as_mut() else {
                continue;
            };
            capture.state = match std::fs::read(&capture.path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => CaptureState::Absent,
                Err(_) => CaptureState::Unreadable,
                Ok(bytes) => match parse_capture(&bytes) {
                    Ok(document) => CaptureState::Collected(document),
                    Err(state) => state,
                },
            };
        }
    }

    /// Reports whether a phase finished with ordinary findings.
    fn is_findings(&self, phase: &PhaseOutcome) -> bool {
        phase.kind == Some(PhaseKind::Findings)
    }

    /// Names every phase that finished with ordinary findings.
    fn with_findings(&self) -> Vec<&'static str> {
        self.phases
            .iter()
            .filter(|phase| self.is_findings(phase))
            .map(|phase| phase.label)
            .collect()
    }

    /// Describes how every planned phase ended.
    fn summary(&self) -> String {
        let parts: Vec<String> = self
            .phases
            .iter()
            .map(|phase| {
                let word = match (phase.state, phase.kind) {
                    (PhaseState::Skipped | PhaseState::Planned, _) => "skipped",
                    (_, Some(PhaseKind::Clean)) => "clean",
                    (_, Some(PhaseKind::Findings)) => "findings",
                    _ => "failed",
                };
                format!("{} {word}", phase.label)
            })
            .collect();
        parts.join(", ")
    }

    /// Names every phase that never started.
    fn skipped(&self) -> Vec<&'static str> {
        self.phases
            .iter()
            .filter(|phase| phase.state == PhaseState::Skipped)
            .map(|phase| phase.label)
            .collect()
    }

    /// Names every phase whose capture could not be collected.
    /// A failed phase that wrote nothing is a tool failure, not a collection failure.
    fn collection_failures(&self) -> Vec<(&'static str, &'static str)> {
        self.phases
            .iter()
            .filter_map(|phase| {
                let capture = phase.capture.as_ref()?;
                let owed = match phase.state {
                    PhaseState::Executed => true,
                    PhaseState::Failed if self.is_findings(phase) => true,
                    PhaseState::Failed => capture.state != CaptureState::Absent,
                    PhaseState::Planned | PhaseState::Skipped => false,
                };
                if !owed {
                    return None;
                }
                Some((phase.label, capture_reason(capture)?))
            })
            .collect()
    }

    /// Names every phase and capture that leaves the merged report incomplete.
    fn reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        for phase in &self.phases {
            let name = phase.label;
            if phase.state == PhaseState::Failed {
                let findings = self.is_findings(phase);
                if !findings {
                    reasons.push(format!("{name} failed"));
                }
                if let Some(reason) = phase
                    .capture
                    .as_ref()
                    .filter(|capture| findings || capture.state != CaptureState::Absent)
                    .and_then(capture_reason)
                {
                    reasons.push(format!("{name} {reason}"));
                }
                continue;
            }
            if phase.state != PhaseState::Executed {
                reasons.push(format!("{name} skipped"));
                continue;
            }
            if let Some(reason) = phase.capture.as_ref().and_then(capture_reason) {
                reasons.push(format!("{name} {reason}"));
            }
        }
        reasons
    }

    /// Reports whether every phase ran and every capture it owed was collected.
    fn complete(&self) -> bool {
        self.reasons().is_empty()
    }

    /// Merges every collected capture into one SARIF document in phase order.
    fn document(&self) -> String {
        let mut runs: Vec<Value> = Vec::new();
        let mut schema = json!(SARIF_SCHEMA);
        let mut version = json!(SARIF_VERSION);
        let mut first = true;
        for phase in &self.phases {
            let Some(CaptureState::Collected(document)) =
                phase.capture.as_ref().map(|capture| &capture.state)
            else {
                continue;
            };
            if first {
                schema = document
                    .get("$schema")
                    .cloned()
                    .unwrap_or_else(|| json!(SARIF_SCHEMA));
                version = document
                    .get("version")
                    .cloned()
                    .unwrap_or_else(|| json!(SARIF_VERSION));
                first = false;
            }
            if let Some(Value::Array(phase_runs)) = document.get("runs") {
                runs.extend(phase_runs.iter().cloned());
            }
        }
        json!({
            "version": version,
            "$schema": schema,
            "runs": runs,
        })
        .to_string()
    }

    /// Deletes every capture a phase wrote.
    fn cleanup(&self) {
        for phase in &self.phases {
            if let Some(capture) = &phase.capture {
                let _ = std::fs::remove_file(&capture.path);
            }
        }
    }
}

/// Names the JSON correlation of one umbrella phase.
fn phase_correlation(command: &str, phase: &str) -> String {
    format!("{command}/{phase}")
}

/// Forwards one finished phase's streams, nesting its JSON lifecycle under the parent command.
/// Returns whether every child line was a readable event.
/// Names the last stderr line of a phase, or says it wrote none.
fn last_stderr_line(phase_err: &[u8]) -> String {
    String::from_utf8_lossy(phase_err)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(|| "no stderr output".to_owned(), str::to_owned)
}

/// Makes a phase that stopped the run as a selection failure say why.
///
/// A phase that ends as `Selection` with a failing exit and no `error` event would otherwise
/// leave only `umbrella_phase_skipped` notices behind. JSON streams gain an `invalid_scope`
/// error before the phase's closing event; text runs gain a stderr line.
fn explain_selection_stop(
    output: OutputMode,
    phase: &str,
    code: i32,
    streams: (&mut Vec<u8>, &mut Vec<u8>),
) {
    let (phase_out, phase_err) = streams;
    let name_of = |line: &[u8]| {
        serde_json::from_slice::<Value>(line)
            .ok()
            .and_then(|event| {
                event
                    .get("event")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
    };
    if output == OutputMode::Json
        && phase_out
            .split(|byte| *byte == b'\n')
            .any(|line| name_of(line).as_deref() == Some("error"))
    {
        return;
    }
    let message = format!(
        "{phase} rejected its scope or configuration (exit {code}) without saying why; \
         last stderr line: {}; pass a Bazel label or package such as //pkg, or use --affected",
        last_stderr_line(phase_err)
    );
    if output != OutputMode::Json {
        if !phase_err.is_empty() {
            return;
        }
        let _ = writeln!(phase_err, "dx: {CODE_INVALID_SCOPE}: {message}");
        return;
    }
    let Ok(event) = error_event(CODE_INVALID_SCOPE, &message, None, None, Some(phase)) else {
        return;
    };
    let mut line = Vec::new();
    if write_event(&mut line, &event).is_err() {
        return;
    }
    let lines: Vec<&[u8]> = phase_out
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    let closing = lines.last().is_some_and(|last| {
        matches!(
            name_of(last).as_deref(),
            Some("phase_finished" | "command_finished")
        )
    });
    let keep = if closing {
        lines.len() - 1
    } else {
        lines.len()
    };
    let mut rebuilt = Vec::new();
    for (index, existing) in lines.iter().enumerate() {
        if index == keep {
            rebuilt.extend_from_slice(&line);
        }
        rebuilt.extend_from_slice(existing);
        rebuilt.push(b'\n');
    }
    if keep == lines.len() {
        rebuilt.extend_from_slice(&line);
    }
    *phase_out = rebuilt;
}

fn forward_phase(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    (command, mode): (&str, &str),
    phase: &str,
    streams: (&[u8], &[u8]),
    code: i32,
) -> Result<bool, i32> {
    let (phase_out, phase_err) = streams;
    if output != OutputMode::Json {
        check_stdout_write(out.write_all(phase_out))?;
        check_stdout_write(err.write_all(phase_err))?;
        return Ok(true);
    }
    check_stdout_write(err.write_all(phase_err))?;
    let correlation = phase_correlation(command, phase);
    let nest = |event: Result<Value, dx_output::OutputError>| {
        event.and_then(|event| nest_phase_event(event, command, phase, &correlation))
    };
    let mut readable = true;
    let mut started = false;
    let mut finished = false;
    for line in phase_out.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let nested = nest(
            std::str::from_utf8(line)
                .map_err(|_| dx_output::OutputError::NotAnEvent)
                .and_then(read_event),
        );
        let Ok(event) = nested else {
            readable = false;
            continue;
        };
        let name = event.get("event").and_then(Value::as_str);
        if !started && name != Some("phase_started") {
            emit_built(out, nest(command_started(phase, mode)))?;
        }
        started = true;
        finished |= name == Some("phase_finished");
        emit_event(out, &event)?;
    }
    if !started {
        emit_built(out, nest(command_started(phase, mode)))?;
    }
    if !readable {
        let message = format!("{phase} wrote an unreadable event");
        let _ = writeln!(err, "dx: {CODE_COLLECTION_FAILED}: {message}");
        emit_built(
            out,
            error_event(CODE_COLLECTION_FAILED, &message, None, None, Some(phase))
                .and_then(|event| with_correlation(event, &correlation)),
        )?;
    }
    if !finished {
        emit_built(
            out,
            nest_phase_event(
                command_finished(code, &FinishedCounts::default()),
                command,
                phase,
                &correlation,
            ),
        )?;
    }
    Ok(readable)
}

/// Adds the phase correlation, or keeps the event without it.
fn correlated(
    built: Result<Value, dx_output::OutputError>,
    command: &str,
    phase: &str,
) -> Result<Value, dx_output::OutputError> {
    built.map(|event| {
        with_correlation(event.clone(), &phase_correlation(command, phase)).unwrap_or(event)
    })
}

/// Announces the phases with findings, the skipped phases, and the failed collections.
fn announce_outcome(
    out: &mut dyn Write,
    output: OutputMode,
    command: &str,
    collector: &PhaseCollector,
    collection_failures: &[(&'static str, String)],
) -> Result<(), i32> {
    for phase in collector.with_findings() {
        announce_findings(out, output, command, phase)?;
    }
    for phase in collector.skipped() {
        announce_skip(out, output, command, phase, &collector.skip_cause)?;
    }
    for (phase, message) in collection_failures {
        announce_collection_failure(out, output, command, phase, message)?;
    }
    Ok(())
}

/// Emits the JSON notice for one phase that never started.
fn announce_skip(
    out: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
    cause: &str,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    let notice = NoticeEvent {
        related_command: Some(command.to_owned()),
        ..NoticeEvent::warning(
            CODE_UMBRELLA_PHASE_SKIPPED,
            format!("{phase} skipped after {cause}"),
        )
    };
    emit_built(out, correlated(notice_event(&notice), command, phase))
}

/// Emits the JSON notice for one phase that finished with ordinary findings.
fn announce_findings(
    out: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    let notice = NoticeEvent {
        related_command: Some(command.to_owned()),
        ..NoticeEvent::warning(
            CODE_UMBRELLA_PHASE_FINDINGS,
            format!("{phase} reported findings"),
        )
    };
    emit_built(out, correlated(notice_event(&notice), command, phase))
}

/// Reports one phase capture the umbrella could not collect.
fn announce_collection_failure(
    out: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
    message: &str,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    emit_built(
        out,
        correlated(
            error_event(CODE_COLLECTION_FAILED, message, None, None, Some(phase)),
            command,
            phase,
        ),
    )
}

/// What re-checking the composed tree after a fix apply found.
struct Verification {
    failed_phase: Option<&'static str>,
    skipped: Vec<&'static str>,
    /// Phases that failed during apply and so were not re-checked.
    apply_failed: Vec<&'static str>,
    /// Phases that were re-checked.
    verified: Vec<&'static str>,
    out: Vec<u8>,
    err: Vec<u8>,
    readable: bool,
    /// The exit code once the buffered stream already ended with `command_finished`.
    ended: Option<i32>,
}

/// Runs the affected tests as the last `dx check --affected` phase.
fn execute_test_phase(test: &Invocation, env: Env<'_>) -> PhaseRun {
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
    let (code, complete, host) = super::workflow::execute_workflow_phase(
        test,
        Env {
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
        },
    );
    PhaseRun {
        code,
        kind: test_phase_kind(code, complete, host),
        effects: PhaseEffects::default(),
    }
}

/// Classifies the test phase from the dx test exit code, result completeness and whether Bazel failed on the host.
fn test_phase_kind(code: i32, complete: bool, host: bool) -> PhaseKind {
    if host && code != 0 {
        return PhaseKind::Host;
    }
    PhaseKind::from_bazel_exit(code, code == BAZEL_TESTS_FAILED && complete)
}

/// Where a `check --apply` verification pass runs, minus its buffered streams.
struct VerifyContext<'a> {
    workspace: &'a Path,
    runner: &'a dyn Runner,
    query_runner: &'a dyn QueryRunner,
    temp_dir: &'a Path,
    pid: u32,
    nonce: u64,
    ci: bool,
    audit_today: Option<&'a str>,
    actions: &'a ActionTally,
    proc_root: &'a Path,
    edit_commit_json: Option<&'a str>,
}

/// Re-runs umbrella phases read-only through `through` after `check --apply` touched them.
fn verify_fix(
    request: &UmbrellaRequest,
    context: &VerifyContext<'_>,
    through: usize,
    failed_during_apply: &[(Command, &'static str)],
) -> Verification {
    let mut outcome = Verification {
        failed_phase: None,
        skipped: Vec::new(),
        apply_failed: failed_during_apply.iter().map(|(_, name)| *name).collect(),
        verified: Vec::new(),
        out: Vec::new(),
        err: Vec::new(),
        readable: true,
        ended: None,
    };
    let command = request.command.name();
    let through = through.min(UMBRELLA_PHASES.len() - 1);
    for (index, phase) in UMBRELLA_PHASES.iter().enumerate().take(through + 1) {
        if failed_during_apply
            .iter()
            .any(|(failed, _)| failed == phase)
        {
            continue;
        }
        if outcome.failed_phase.is_some() {
            outcome.skipped.push(phase.name());
            continue;
        }
        outcome.verified.push(phase.name());
        let mut phase_out = Vec::new();
        let mut phase_err = Vec::new();
        let code = {
            let phase_env = Env {
                workspace: context.workspace,
                runner: context.runner,
                query_runner: context.query_runner,
                temp_dir: context.temp_dir,
                pid: context.pid,
                nonce: context.nonce.wrapping_add(index as u64),
                out: &mut phase_out,
                err: &mut phase_err,
                ci: context.ci,
                audit_today: context.audit_today,
                actions: context.actions,
                proc_root: context.proc_root,
                edit_commit_json: context.edit_commit_json,
            };
            if *phase == Command::Generate {
                execute_generate_request(&request.verify_generate_request(), phase_env)
            } else {
                match request.verify_quality_request(*phase) {
                    Ok(phase_request) => execute_quality_request(&phase_request, phase_env),
                    Err(message) => pre_exec(
                        request.common.output,
                        phase_env.out,
                        phase_env.err,
                        message.message(),
                    ),
                }
            }
        };
        let verify_phase = format!("verify/{}", phase.name());
        let forwarded = forward_phase(
            &mut outcome.out,
            &mut outcome.err,
            request.common.output,
            (command, "check"),
            &verify_phase,
            (&phase_out, &phase_err),
            code,
        );
        match forwarded {
            Ok(readable) => outcome.readable &= readable,
            Err(_) => {
                outcome.readable = false;
                let message = format!("{verify_phase} events could not be forwarded");
                let _ = writeln!(outcome.err, "dx: {CODE_COLLECTION_FAILED}: {message}");
                if let Err(exit) = announce_collection_failure(
                    &mut outcome.out,
                    request.common.output,
                    command,
                    &verify_phase,
                    &message,
                ) {
                    outcome.ended = Some(exit);
                    break;
                }
            }
        }
        if code != 0 {
            outcome.failed_phase = Some(phase.name());
        }
    }
    outcome
}

/// Delivers the buffered verification output when verification fails the run.
fn deliver_verification(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    command: &str,
    verification: &Verification,
) -> Result<(), i32> {
    let partial = !verification.apply_failed.is_empty() && !verification.verified.is_empty();
    if verification.failed_phase.is_none() && verification.readable && !partial {
        return Ok(());
    }
    check_stdout_write(out.write_all(&verification.out))?;
    check_stdout_write(err.write_all(&verification.err))?;
    if let Some(exit) = verification.ended {
        return Err(exit);
    }
    if let Some(failed) = verification.failed_phase {
        announce_verification_failure(out, err, output, command, failed, &verification.skipped)?;
    } else if partial {
        let _ = writeln!(
            err,
            "dx: verification passed after apply for {}; not re-checked, failed during apply: {}",
            verification.verified.join(", "),
            verification.apply_failed.join(", ")
        );
    }
    Ok(())
}

/// Announces one fix apply whose read-only verification found drift.
fn announce_verification_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
    skipped: &[&str],
) -> Result<(), i32> {
    let mut message = format!("{phase} failed verification after apply");
    if !skipped.is_empty() {
        message.push_str(&format!(
            "; verification skipped for {}",
            skipped.join(", ")
        ));
    }
    let _ = writeln!(err, "dx: {CODE_VERIFICATION_FAILED}: {message}");
    if output != OutputMode::Json {
        return Ok(());
    }
    emit_built(
        out,
        correlated(
            error_event(CODE_VERIFICATION_FAILED, &message, None, None, Some(phase)),
            command,
            phase,
        ),
    )
}

/// Announces one written report and names why it is incomplete.
fn announce_report(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    format: &str,
    destination: &str,
    complete: bool,
    reasons: &[String],
) -> Result<(), i32> {
    let note = if complete {
        format!("Wrote {format} report to {destination}.")
    } else {
        format!(
            "Wrote {format} report to {destination} (incomplete: {}).",
            reasons.join(", ")
        )
    };
    if output == OutputMode::Json {
        emit_built(out, report_event(format, destination, complete))?;
    } else if matches!(output, OutputMode::Text { .. }) {
        check_stdout_write(writeln!(out, "{note}"))?;
    } else if output == OutputMode::Diff {
        check_stdout_write(writeln!(err, "{note}"))?;
    }
    Ok(())
}

pub(crate) fn execute_umbrella(invocation: &Invocation, env: Env<'_>) -> i32 {
    let request = match invocation.umbrella_request() {
        Ok(request) => request,
        Err(message) => {
            let Env { out, err, .. } = env;
            return pre_exec(invocation.output, out, err, message.message());
        }
    };
    execute_umbrella_request(&request, env)
}

pub(crate) fn execute_umbrella_request(request: &UmbrellaRequest, env: Env<'_>) -> i32 {
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
    let code = run_umbrella(
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
    );
    latched.exit_code(code)
}

fn run_umbrella(request: &UmbrellaRequest, env: Env<'_>) -> i32 {
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
    let fix_apply = request.applies();
    // One rule-graph load serves every phase until a phase may have rewritten BUILD files.
    let graph_memo = MemoizedGraphQuery::new(query_runner);
    let query_runner: &dyn QueryRunner = &graph_memo;
    let phase_check = !fix_apply;
    let mode = if phase_check { "check" } else { "default" };
    let command = request.command.name();
    if let Err(error) = plan_reports(
        workspace,
        request.command,
        &request.common.reports,
        &request.common.output,
    ) {
        return pre_exec(request.common.output, out, err, &error.to_string());
    }
    for report in &request.common.reports {
        if report.destination == "-" {
            return pre_exec(
                request.common.output,
                out,
                err,
                &format!(
                    "option \"--report={}={}\" is not supported by dx {}: phases share one stdout document",
                    report.format, report.destination, command,
                ),
            );
        }
    }
    if let Some(exit) = guard_build_output(workspace, request.common.output, out, err) {
        return exit;
    }
    if request.common.output == OutputMode::Json
        && let Err(exit) = emit_built(out, quality_started(command, mode, request.unsafe_fixes))
    {
        return exit;
    }
    let mut phases: Vec<(Command, &'static str)> = UMBRELLA_PHASES
        .iter()
        .map(|phase| (*phase, phase.name()))
        .collect();
    if request.test.is_some() {
        phases.push((Command::Test, Command::Test.name()));
    }
    let mut collector = PhaseCollector::default();
    for (phase, label) in &phases {
        collector.plan(*phase, label);
    }
    let mut stop_code: Option<i32> = None;
    let mut failed_code: Option<i32> = None;
    let mut skip_cause = String::from("an earlier phase failed");
    let mut stdout_exit: Option<i32> = None;
    let mut reports_ok = true;
    let mut last_touched: Option<usize> = None;
    let mut failed_phases: Vec<(Command, &'static str)> = Vec::new();
    let mut quality_applied = false;
    let mut generate_clean = false;
    let mut rerun_planned = false;
    let mut index = 0;
    while index < phases.len() {
        let (phase, label) = phases[index];
        let phase_nonce = nonce.wrapping_add(index as u64);
        let mut phase_reports = Vec::new();
        let mut sarif_capture: Option<PathBuf> = None;
        for report in &request.common.reports {
            if phase == Command::Test || !spec(phase).accepts_report(&report.format) {
                continue;
            }
            if sarif_capture.is_none() {
                let capture = temp_dir.join(format!(
                    "umbrella-{}-{pid}-{phase_nonce}.sarif",
                    phase.name()
                ));
                let Some(capture_text) = capture.to_str() else {
                    return pre_exec(
                        request.common.output,
                        out,
                        err,
                        "temporary report path is not UTF-8",
                    ); // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                };
                phase_reports.push(ReportRequest {
                    format: report.format.clone(),
                    destination: capture_text.to_owned(),
                });
                sarif_capture = Some(capture);
            }
        }
        if let Some(capture) = sarif_capture {
            collector.require_capture(index, capture);
        }
        let mut phase_out = Vec::new();
        let mut phase_err = Vec::new();
        let PhaseRun {
            code,
            mut kind,
            effects,
        } = {
            let phase_env = Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce: phase_nonce,
                out: &mut phase_out,
                err: &mut phase_err,
                ci,
                audit_today,
                actions,
                proc_root,
                edit_commit_json,
            };
            if phase == Command::Generate {
                let generate = request.phase_generate_request(phase_reports);
                execute_generate_phase(&generate, phase_env)
            } else if let (Command::Test, Some(test)) = (phase, request.test.as_deref()) {
                execute_test_phase(test, phase_env)
            } else {
                match request.phase_quality_request(phase, phase_reports) {
                    Ok(phase_request) => execute_quality_phase(&phase_request, phase_env),
                    Err(message) if request.common.output == OutputMode::Json => PhaseRun {
                        code: pre_exec(
                            request.common.output,
                            phase_env.out,
                            phase_env.err,
                            message.message(),
                        ),
                        kind: PhaseKind::Selection,
                        effects: PhaseEffects::default(),
                    },
                    Err(message) => {
                        let _ = writeln!(phase_env.err, "dx: {message}");
                        PhaseRun {
                            code: 2,
                            kind: PhaseKind::Selection,
                            effects: PhaseEffects::default(),
                        }
                    }
                }
            }
        };
        if code != 0 && interrupted().is_some() {
            kind = PhaseKind::Cancelled;
        }
        if effects.touched() {
            last_touched = Some(index);
        }
        if fix_apply && (code != 0 || effects.changes + effects.applied + effects.not_applied > 0) {
            graph_memo.invalidate();
        }
        if phase == Command::Generate {
            generate_clean |= code == 0 && label == phase.name();
        } else {
            quality_applied |= effects.applied > 0;
        }
        if kind == PhaseKind::Selection && code != 0 {
            explain_selection_stop(
                request.common.output,
                label,
                code,
                (&mut phase_out, &mut phase_err),
            );
        }
        collector.record_exit(index, code, kind);
        match forward_phase(
            out,
            err,
            request.common.output,
            (command, mode),
            label,
            (&phase_out, &phase_err),
            code,
        ) {
            Ok(readable) => reports_ok &= readable,
            Err(exit) => {
                stdout_exit = Some(exit);
                skip_cause = String::from("output delivery failed");
                break;
            }
        }
        index += 1;
        if code != 0 {
            let failed_rerun = kind == PhaseKind::GenerateFailed && label == RERUN_GENERATE;
            if !kind.continues(fix_apply) && !failed_rerun {
                stop_code = Some(code);
                skip_cause = format!("{label} {}", kind.stop_reason());
                if kind == PhaseKind::Compile
                    && fix_apply
                    && generate_clean
                    && quality_applied
                    && !rerun_planned
                {
                    // A compile error may need a dep that generate adds, so
                    // skip only the phases after this one and rerun generate.
                    rerun_planned = true;
                    collector.skip_remaining(skip_cause.clone());
                    phases.push((Command::Generate, RERUN_GENERATE));
                    collector.plan(Command::Generate, RERUN_GENERATE);
                    index = phases.len() - 1;
                    continue;
                }
                break;
            }
            failed_code.get_or_insert(code);
            failed_phases.push((phase, label));
        }
        if index == phases.len() && fix_apply && generate_clean && quality_applied && !rerun_planned
        {
            rerun_planned = true;
            phases.push((Command::Generate, RERUN_GENERATE));
            collector.plan(Command::Generate, RERUN_GENERATE);
        }
    }
    collector.skip_remaining(skip_cause);
    collector.collect();
    let complete = collector.complete();
    let reasons = collector.reasons();
    let collection_failures: Vec<(&'static str, String)> = collector
        .collection_failures()
        .into_iter()
        .map(|(phase, reason)| {
            let message = format!("{phase} {reason}");
            let _ = writeln!(err, "dx: {CODE_COLLECTION_FAILED}: {message}");
            (phase, message)
        })
        .collect();
    reports_ok &= collection_failures.is_empty();
    if stdout_exit.is_none() {
        let announced = announce_outcome(
            out,
            request.common.output,
            command,
            &collector,
            &collection_failures,
        );
        if let Err(exit) = announced {
            stdout_exit = Some(exit);
        }
    }
    if stdout_exit.is_none() {
        let document = collector.document();
        for report in &request.common.reports {
            if let Err(error) = write_report_file(workspace, &report.destination, &document) {
                reports_ok = false;
                if let Err(exit) = report_failed(
                    out,
                    err,
                    request.common.output,
                    &format!("failed to write {} report to {error}", report.format),
                ) {
                    stdout_exit = Some(exit);
                    break;
                }
                continue;
            }
            let announced = announce_report(
                out,
                err,
                request.common.output,
                &report.format,
                &report.destination,
                complete,
                &reasons,
            );
            if let Err(exit) = announced {
                stdout_exit = Some(exit);
                break;
            }
        }
    }
    collector.cleanup();
    if !matches!(request.common.output, OutputMode::Json)
        && (stop_code.is_some() || failed_code.is_some())
    {
        let _ = writeln!(err, "dx: phases: {}", collector.summary());
    }
    let mut verify_forward_exit: Option<i32> = None;
    let mut verification_failed = false;
    let verify_through = if fix_apply && stdout_exit.is_none() && stop_code.is_none() {
        last_touched
    } else {
        None
    };
    if let Some(through) = verify_through {
        graph_memo.invalidate();
        let context = VerifyContext {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid,
            nonce: nonce.wrapping_add(phases.len() as u64),
            ci,
            audit_today,
            actions,
            proc_root,
            edit_commit_json,
        };
        let verification = verify_fix(request, &context, through, &failed_phases);
        reports_ok &= verification.readable;
        verification_failed = verification.failed_phase.is_some();
        if let Err(exit) =
            deliver_verification(out, err, request.common.output, command, &verification)
        {
            verify_forward_exit = Some(exit);
        }
    }
    let mut commits_flagged = false;
    if stdout_exit.is_none() && !fix_apply {
        match lint_commit_subjects(request, workspace, out, err) {
            Ok(flagged) => commits_flagged = flagged,
            Err(exit) => stdout_exit = Some(exit),
        }
    }
    let code = match stdout_exit {
        Some(exit) => exit,
        None => match verify_forward_exit {
            Some(exit) => exit,
            None => match stop_code.or(failed_code) {
                Some(phase_code) => phase_code,
                None if reports_ok && !verification_failed && !commits_flagged => 0,
                None => 1,
            },
        },
    };
    if request.common.output == OutputMode::Json
        && stdout_exit.is_none()
        && verify_forward_exit.is_none()
    {
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(complete),
                actions: actions.total(),
                ..FinishedCounts::default()
            },
        );
        let _ = write_event(out, &finished);
    }
    code
}

/// Warns about each commit subject since the merge base that is not a conventional commit.
/// Runs only when `dx check --affected` found a merge base and `dx.toml` sets `[check] commits`.
fn lint_commit_subjects(
    request: &UmbrellaRequest,
    workspace: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let Some(base) = request.commits_base.as_deref() else {
        return Ok(false);
    };
    lint_commits(
        base,
        request.command,
        request.common.output,
        workspace,
        out,
        err,
    )
}

/// Lints the commit subjects in `base..HEAD` when `dx.toml` sets `[check] commits`.
pub(super) fn lint_commits(
    base: &str,
    command: Command,
    output: OutputMode,
    workspace: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let enabled = matches!(
        dx_config::load(workspace),
        Ok(Some(config)) if config.check.commits == Some(dx_config::CommitsPolicy::Conventional)
    );
    if !enabled {
        return Ok(false);
    }
    let range = format!("{base}..HEAD");
    let args = [&dx_update::conventional::LOG_ARGS[..], &[range.as_str()]].concat();
    let Some(log) = git(workspace, &args) else {
        let _ = writeln!(
            err,
            "dx: warning: commits: cannot read the commits in {range}"
        );
        return Ok(false);
    };
    let bad = dx_update::conventional::lint(&log);
    let command = command.name();
    for (hash, subject) in &bad {
        let sha = &hash[..hash.len().min(12)];
        let message = format!("{sha} {subject}: not a conventional commit");
        if output == OutputMode::Json {
            let notice = NoticeEvent {
                related_command: Some(command.to_owned()),
                ..NoticeEvent::warning(CODE_COMMIT_NOT_CONVENTIONAL, message)
            };
            emit_built(out, correlated(notice_event(&notice), command, "commits"))?;
        } else {
            let _ = writeln!(err, "dx: warning: commits: {message}");
        }
    }
    Ok(!bad.is_empty())
}

fn git(workspace: &Path, args: &[&str]) -> Option<String> {
    let mut command = dx_process::capture::git(workspace);
    command.args(args);
    dx_process::capture::run_checked(&mut command, dx_process::capture::DEFAULT_TIMEOUT)
        .ok()
        .map(|output| output.stdout_text())
}

#[cfg(test)]
#[path = "umbrella_tests.rs"]
mod tests;
