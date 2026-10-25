use super::common::*;
use super::generate::execute_generate_phase;
use super::generate::execute_generate_request;
use super::quality::execute_quality_request;
use super::quality::{execute_quality_phase, operational_build_output};
use crate::args::{Command, Invocation, ReportRequest, UmbrellaRequest};
use crate::plan::spec;
use crate::reports::plan_reports;
use crate::resolve::QueryRunner;
use dx_output::{
    command_finished, command_started, error_event, nest_phase_event, notice_event, read_event,
    report_event, with_correlation, write_event, FinishedCounts, NoticeEvent, OutputMode,
};
use dx_process::{interrupted, BoundedOutcome, ChildStatus, Runner, TimeBound};
use serde_json::{json, Value};
use serde_sarif::sarif::Sarif;
use std::cell::Cell;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const UMBRELLA_PHASES: [Command; 4] = [
    Command::Format,
    Command::Lint,
    Command::Typecheck,
    Command::Generate,
];

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
    /// Records one phase as planned and waiting to run.
    fn plan(&mut self, phase: Command) {
        self.phases.push(PhaseOutcome {
            phase,
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
            .map(|phase| phase.phase.name())
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
                format!("{} {word}", phase.phase.name())
            })
            .collect();
        parts.join(", ")
    }

    /// Names every phase that never started.
    fn skipped(&self) -> Vec<&'static str> {
        self.phases
            .iter()
            .filter(|phase| phase.state == PhaseState::Skipped)
            .map(|phase| phase.phase.name())
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
                Some((phase.phase.name(), capture_reason(capture)?))
            })
            .collect()
    }

    /// Names every phase and capture that leaves the merged report incomplete.
    fn reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        for phase in &self.phases {
            let name = phase.phase.name();
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
            emit_event(out, &nest(command_started(phase, mode)).map_err(|_| 1)?)?;
        }
        started = true;
        finished |= name == Some("phase_finished");
        emit_event(out, &event)?;
    }
    if !started {
        emit_event(out, &nest(command_started(phase, mode)).map_err(|_| 1)?)?;
    }
    if !readable {
        let message = format!("{phase} wrote an unreadable event");
        let _ = writeln!(err, "dx: {CODE_COLLECTION_FAILED}: {message}");
        if let Ok(event) = error_event(CODE_COLLECTION_FAILED, &message, None, None, Some(phase)) {
            emit_event(out, &with_correlation(event, &correlation).map_err(|_| 1)?)?;
        }
    }
    if !finished {
        let event = nest_phase_event(
            command_finished(code, &FinishedCounts::default()),
            command,
            phase,
            &correlation,
        )
        .map_err(|_| 1)?;
        emit_event(out, &event)?;
    }
    Ok(readable)
}

/// Emits the JSON notice for one phase that never started.
fn announce_skip(out: &mut dyn Write, output: OutputMode, command: &str, phase: &str, cause: &str) {
    if output != OutputMode::Json {
        return;
    }
    let notice = NoticeEvent {
        level: "warning".to_owned(),
        code: "umbrella_phase_skipped".to_owned(),
        message: format!("{phase} skipped after {cause}"),
        related_command: Some(command.to_owned()),
        scope: None,
        path: None,
        language: None,
        import: None,
    };
    if let Ok(event) = notice_event(&notice) {
        let correlated =
            with_correlation(event.clone(), &phase_correlation(command, phase)).unwrap_or(event);
        let _ = write_event(out, &correlated);
    }
}

/// Emits the JSON notice for one phase that finished with ordinary findings.
fn announce_findings(out: &mut dyn Write, output: OutputMode, command: &str, phase: &str) {
    if output != OutputMode::Json {
        return;
    }
    let notice = NoticeEvent {
        level: "warning".to_owned(),
        code: "umbrella_phase_findings".to_owned(),
        message: format!("{phase} reported findings"),
        related_command: Some(command.to_owned()),
        scope: None,
        path: None,
        language: None,
        import: None,
    };
    if let Ok(event) = notice_event(&notice) {
        let correlated =
            with_correlation(event.clone(), &phase_correlation(command, phase)).unwrap_or(event);
        let _ = write_event(out, &correlated);
    }
}

/// Reports one phase capture the umbrella could not collect.
fn announce_collection_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
    reason: &str,
) {
    let message = format!("{phase} {reason}");
    let _ = writeln!(err, "dx: {CODE_COLLECTION_FAILED}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_COLLECTION_FAILED, &message, None, None, Some(phase)) {
            let correlated = with_correlation(event.clone(), &phase_correlation(command, phase))
                .unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    }
}

/// What re-checking the composed tree after a fix apply found.
struct Verification {
    failed_phase: Option<&'static str>,
    skipped: Vec<&'static str>,
    out: Vec<u8>,
    err: Vec<u8>,
    readable: bool,
}

const BAZEL_BUILD_FAILED: i32 = 1;
const BAZEL_USAGE: i32 = 2;
const BAZEL_TESTS_FAILED: i32 = 3;
const BAZEL_NO_TESTS: i32 = 4;
const BAZEL_INTERRUPTED: i32 = 8;

/// Runner that forwards every call and records whether Bazel failed to launch or was signalled.
struct LaunchWatch<'a> {
    inner: &'a dyn Runner,
    failed: Cell<bool>,
}

impl<'a> LaunchWatch<'a> {
    fn new(inner: &'a dyn Runner) -> Self {
        Self {
            inner,
            failed: Cell::new(false),
        }
    }

    fn failed(&self) -> bool {
        self.failed.get()
    }

    fn watch(&self, result: io::Result<ChildStatus>) -> io::Result<ChildStatus> {
        if !matches!(result, Ok(ChildStatus { code: Some(_) })) {
            self.failed.set(true);
        }
        result
    }
}

impl Runner for LaunchWatch<'_> {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.watch(self.inner.run(argv, cwd, env))
    }

    fn run_bounded(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        bound: TimeBound,
    ) -> io::Result<BoundedOutcome> {
        self.inner.run_bounded(argv, cwd, env, bound)
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.watch(self.inner.run_hermetic(argv, cwd, env))
    }

    fn run_withdrawable(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<ChildStatus>> {
        match self.inner.run_withdrawable(argv, cwd, env, withdraw) {
            Ok(None) => Ok(None),
            Ok(Some(status)) => self.watch(Ok(status)).map(Some),
            Err(error) => self.watch(Err(error)).map(Some),
        }
    }

    fn gitleaks_tool(&self) -> Option<PathBuf> {
        self.inner.gitleaks_tool()
    }

    fn pinned_tool(&self, var: &str) -> Option<PathBuf> {
        self.inner.pinned_tool(var)
    }

    fn tool_version(&self, path: &Path) -> Option<String> {
        self.inner.tool_version(path)
    }

    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }
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
    } = env;
    let watch = LaunchWatch::new(runner);
    let (code, complete) = super::workflow::execute_workflow_phase(
        test,
        Env {
            workspace,
            runner: &watch,
            query_runner,
            temp_dir,
            pid,
            nonce,
            out,
            err,
            ci,
        },
    );
    PhaseRun {
        code,
        kind: test_phase_kind(code, complete, watch.failed()),
        effects: PhaseEffects::default(),
    }
}

/// Classifies the test phase from the dx test exit code, result completeness and whether Bazel launched.
fn test_phase_kind(code: i32, complete: bool, launch_failed: bool) -> PhaseKind {
    if launch_failed && code != 0 {
        return PhaseKind::Host;
    }
    match code {
        0 => PhaseKind::Clean,
        BAZEL_TESTS_FAILED if complete => PhaseKind::Findings,
        BAZEL_BUILD_FAILED | BAZEL_TESTS_FAILED => PhaseKind::Incomplete,
        BAZEL_USAGE | BAZEL_NO_TESTS => PhaseKind::Selection,
        BAZEL_INTERRUPTED => PhaseKind::Cancelled,
        _ => PhaseKind::Tool,
    }
}

/// Where a `fix --apply` verification pass runs, minus its buffered streams.
struct VerifyContext<'a> {
    workspace: &'a Path,
    runner: &'a dyn Runner,
    query_runner: &'a dyn QueryRunner,
    temp_dir: &'a Path,
    pid: u32,
    nonce: u64,
    ci: bool,
}

/// Re-runs umbrella phases read-only through `through` after `fix --apply` touched them.
fn verify_fix(
    request: &UmbrellaRequest,
    context: &VerifyContext<'_>,
    through: usize,
) -> Verification {
    let mut outcome = Verification {
        failed_phase: None,
        skipped: Vec::new(),
        out: Vec::new(),
        err: Vec::new(),
        readable: true,
    };
    let command = request.command.name();
    for (index, phase) in UMBRELLA_PHASES.iter().enumerate().take(through + 1) {
        if outcome.failed_phase.is_some() {
            outcome.skipped.push(phase.name());
            continue;
        }
        let mut phase_out = Vec::new();
        let mut phase_err = Vec::new();
        let code = {
            let phase_env = Env {
                workspace: context.workspace,
                runner: context.runner,
                query_runner: context.query_runner,
                temp_dir: context.temp_dir,
                pid: context.pid,
                nonce: context
                    .nonce
                    .wrapping_add(UMBRELLA_PHASES.len() as u64)
                    .wrapping_add(index as u64),
                out: &mut phase_out,
                err: &mut phase_err,
                ci: context.ci,
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
                        &message,
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
        outcome.readable &= forwarded.unwrap_or(false);
        if code != 0 {
            outcome.failed_phase = Some(phase.name());
        }
    }
    outcome
}

/// Announces one fix apply whose read-only verification found drift.
fn announce_verification_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
    skipped: &[&str],
) {
    let mut message = format!("{phase} failed verification after apply");
    if !skipped.is_empty() {
        message.push_str(&format!(
            "; verification skipped for {}",
            skipped.join(", ")
        ));
    }
    let _ = writeln!(err, "dx: {CODE_VERIFICATION_FAILED}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_VERIFICATION_FAILED, &message, None, None, Some(phase))
        {
            let correlated = with_correlation(event.clone(), &phase_correlation(command, phase))
                .unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    }
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
        if let Ok(event) = report_event(format, destination, complete) {
            emit_event(out, &event)?;
        }
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
            return pre_exec(invocation.output, out, err, &message);
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
    } = env;
    let fix_apply = request.command == Command::Fix && request.applies();
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
    let build_output = dx_adopt::build_output::scan(workspace);
    if !build_output.is_empty() {
        let lines: Vec<String> = build_output
            .iter()
            .map(|finding| format!("{}: {}", finding.path, finding.reason))
            .collect();
        return operational_build_output(request.common.output, out, err, &lines);
    }
    if request.common.output == OutputMode::Json {
        if let Ok(event) = command_started(command, mode) {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
    }
    let mut phases: Vec<Command> = UMBRELLA_PHASES.to_vec();
    if request.test.is_some() {
        phases.push(Command::Test);
    }
    let mut collector = PhaseCollector::default();
    for phase in &phases {
        collector.plan(*phase);
    }
    let mut stop_code: Option<i32> = None;
    let mut failed_code: Option<i32> = None;
    let mut skip_cause = String::from("an earlier phase failed");
    let mut stdout_exit: Option<i32> = None;
    let mut reports_ok = true;
    let mut last_touched: Option<usize> = None;
    for (index, phase) in phases.iter().enumerate() {
        let phase_nonce = nonce.wrapping_add(index as u64);
        let mut phase_reports = Vec::new();
        let mut sarif_capture: Option<PathBuf> = None;
        for report in &request.common.reports {
            if *phase == Command::Test || !spec(*phase).accepts_report(&report.format) {
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
            };
            if *phase == Command::Generate {
                execute_generate_phase(&request.phase_generate_request(phase_reports), phase_env)
            } else if let (Command::Test, Some(test)) = (*phase, request.test.as_deref()) {
                execute_test_phase(test, phase_env)
            } else {
                match request.phase_quality_request(*phase, phase_reports) {
                    Ok(phase_request) => execute_quality_phase(&phase_request, phase_env),
                    Err(message) if request.common.output == OutputMode::Json => PhaseRun {
                        code: pre_exec(
                            request.common.output,
                            phase_env.out,
                            phase_env.err,
                            &message,
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
        collector.record_exit(index, code, kind);
        match forward_phase(
            out,
            err,
            request.common.output,
            (command, mode),
            phase.name(),
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
        if code != 0 {
            if kind.continues() {
                failed_code.get_or_insert(code);
                continue;
            }
            stop_code = Some(code);
            skip_cause = format!("{} {}", phase.name(), kind.stop_reason());
            break;
        }
    }
    collector.skip_remaining(skip_cause);
    collector.collect();
    let complete = collector.complete();
    let reasons = collector.reasons();
    for phase in collector.with_findings() {
        announce_findings(out, request.common.output, command, phase);
    }
    for phase in collector.skipped() {
        announce_skip(
            out,
            request.common.output,
            command,
            phase,
            &collector.skip_cause,
        );
    }
    for (phase, reason) in collector.collection_failures() {
        reports_ok = false;
        announce_collection_failure(out, err, request.common.output, command, phase, reason);
    }
    if stdout_exit.is_none() {
        let document = collector.document();
        for report in &request.common.reports {
            if let Err(error) = write_report_file(workspace, &report.destination, &document) {
                reports_ok = false;
                report_failed(
                    out,
                    err,
                    request.common.output,
                    &format!("failed to write {} report to {error}", report.format),
                );
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
    let verify_through =
        if fix_apply && stdout_exit.is_none() && stop_code.is_none() && failed_code.is_none() {
            last_touched
        } else {
            None
        };
    if let Some(through) = verify_through {
        let context = VerifyContext {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid,
            nonce,
            ci,
        };
        let verification = verify_fix(request, &context, through);
        reports_ok &= verification.readable;
        if let Some(failed) = verification.failed_phase {
            verification_failed = true;
            if let Err(exit) = check_stdout_write(out.write_all(&verification.out))
                .and_then(|()| check_stdout_write(err.write_all(&verification.err)))
            {
                verify_forward_exit = Some(exit);
            } else {
                announce_verification_failure(
                    out,
                    err,
                    request.common.output,
                    command,
                    failed,
                    &verification.skipped,
                );
            }
        }
    }
    let code = match stdout_exit {
        Some(exit) => exit,
        None => match verify_forward_exit {
            Some(exit) => exit,
            None => match stop_code.or(failed_code) {
                Some(phase_code) => phase_code,
                None if reports_ok && !verification_failed => 0,
                None => 1,
            },
        },
    };
    if request.common.output == OutputMode::Json {
        let finished = command_finished(
            code,
            &FinishedCounts {
                results_complete: Some(complete),
                ..FinishedCounts::default()
            },
        );
        let _ = write_event(out, &finished);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::args::Invocation;
    use crate::plan::GENERATE_ENV_INTENDED;
    use dx_process::{ChildStatus, Runner};
    use std::cell::{Cell, RefCell};
    use std::io::{self, Write};
    use std::path::Path;
    use std::rc::Rc;

    fn setting_args(argv: &[String]) -> std::collections::BTreeSet<String> {
        argv.iter()
            .filter(|arg| arg.starts_with("--@"))
            .cloned()
            .collect()
    }

    #[test]
    fn umbrella_phases_and_test_share_the_bazel_settings() {
        let resolved = crate::resolve::ResolvedScope {
            scope: dx_process::Scope::Repository,
            targets: Vec::new(),
        };
        let shared: std::collections::BTreeSet<String> = crate::plan::BAZEL_SETTINGS
            .iter()
            .map(ToString::to_string)
            .collect();
        let mut argvs = Vec::new();
        for phase in UMBRELLA_PHASES {
            let plan = if phase == Command::Generate {
                crate::plan::plan_generate(&resolved, &[], true, &[])
            } else {
                crate::plan::plan_build(phase, &resolved, &[], "/tmp/bep.json", &[])
            };
            argvs.push((format!("{phase:?}"), plan.expect("phase plan").argv));
        }
        let test = crate::plan::plan_workflow(
            crate::plan::WorkflowVerb::Test,
            &resolved,
            &[],
            Some("/tmp/bep.json"),
            Some(crate::args::Profile::Dev),
            &[],
        )
        .expect("test plan");
        argvs.push(("test".to_owned(), test.argv));
        let profile = crate::args::Profile::Dev;
        let label = "//app:bin".to_owned();
        let planned = [
            ("run", crate::plan::plan_run(&label, &[], &[], profile, &[])),
            (
                "run build",
                crate::plan::plan_run_build(std::slice::from_ref(&label), &[], profile, &[]),
            ),
            (
                "deploy build",
                crate::plan::plan_deploy_build(&label, profile, &[]),
            ),
            (
                "deploy run",
                crate::plan::plan_deploy_run(&label, &[], profile, &[]),
            ),
        ];
        for (name, plan) in planned {
            argvs.push((name.to_owned(), plan.expect("plan").argv));
        }
        for command in [Command::Codegen, Command::Env] {
            let plan = crate::plan::plan_managed(
                command,
                &dx_setup::SetupScope::Repository,
                &[],
                "/tmp/bep.json",
                &[],
            )
            .expect("managed plan");
            argvs.push((format!("{command:?}"), plan.argv));
        }
        let selection = crate::resolve::SelectionContext::for_test_execution(
            crate::args::Profile::Dev,
            &[],
            &[],
        );
        let cquery = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("cquery argv");
        argvs.push(("cquery".to_owned(), cquery));
        for (name, argv) in &argvs {
            assert_eq!(setting_args(argv), shared, "{name}: {argv:?}");
        }
        let query = selection
            .selection_argv("query", &[], "tests(//...)")
            .expect("query argv");
        assert!(setting_args(&query).is_empty(), "{query:?}");
    }

    /// Names the capture one umbrella phase writes into the harness temp dir.
    fn capture_path(harness: &Harness, phase: &str, index: u64) -> PathBuf {
        harness.temp.join(format!(
            "umbrella-{phase}-{}-{index}.sarif",
            std::process::id()
        ))
    }

    /// Parses one umbrella invocation against a harness workspace.
    fn umbrella_invocation(harness: &Harness, words: &[&str]) -> Invocation {
        let parsed = invocation(words);
        crate::args::apply_here(&parsed, &harness.workspace, &harness.cwd)
            .expect("resolve workspace")
    }

    /// How one phase capture is rewritten once its phase has finished.
    #[derive(Clone, Copy)]
    enum Rewrite {
        Remove,
        Block,
        Bytes(&'static [u8]),
    }

    impl Rewrite {
        fn apply(self, path: &Path) {
            match self {
                Rewrite::Remove => {
                    std::fs::remove_file(path).expect("remove capture");
                }
                Rewrite::Block => {
                    std::fs::remove_file(path).expect("remove capture");
                    std::fs::create_dir(path).expect("capture directory");
                }
                Rewrite::Bytes(payload) => {
                    std::fs::write(path, payload).expect("rewrite capture");
                }
            }
        }
    }

    /// Serves the inner runner but exits nonzero from one launch up to an optional end.
    struct FailFrom<R> {
        inner: R,
        from: usize,
        to: Option<usize>,
        code: i32,
        launches: Cell<usize>,
    }

    impl<R: Runner> Runner for FailFrom<R> {
        fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
            self.inner.free_disk_bytes(path)
        }

        fn run(
            &self,
            argv: &[String],
            cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let launch = self.launches.get();
            self.launches.set(launch + 1);
            let status = self.inner.run(argv, cwd, env)?;
            if launch >= self.from && self.to.is_none_or(|to| launch < to) {
                return Ok(ChildStatus {
                    code: Some(self.code),
                });
            }
            Ok(status)
        }
    }

    /// Rewrites one phase capture once the phase that wrote it has finished.
    struct RewriteCapture<R> {
        inner: R,
        capture: PathBuf,
        rewrite: Rewrite,
        done: Cell<bool>,
    }

    impl<R: Runner> Runner for RewriteCapture<R> {
        fn run(
            &self,
            argv: &[String],
            cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let status = self.inner.run(argv, cwd, env)?;
            if !self.done.get() && self.capture.exists() {
                self.done.set(true);
                self.rewrite.apply(&self.capture);
            }
            Ok(status)
        }
    }

    /// Runs one umbrella command whose lint capture is rewritten mid-run.
    fn umbrella_with_broken_capture(
        harness: Harness,
        rewrite: Rewrite,
        words: &[&str],
    ) -> (Harness, i32, String, String) {
        let capture = capture_path(&harness, "lint", 1);
        let runner = RewriteCapture {
            inner: harness.runner(),
            capture,
            rewrite,
            done: Cell::new(false),
        };
        let inv = umbrella_invocation(&harness, words);
        let (code, out, err) = harness.execute_with(&inv, &runner);
        (harness, code, out, err)
    }

    /// Serves apply content for the first `verify_from` launches, then
    /// verification content, so `fix --apply` tests can stage what the
    /// read-only re-check observes after the apply pass wrote sources.
    struct StagedRunner {
        code: Option<i32>,
        bep_lines: Vec<String>,
        apply_files: Vec<(PathBuf, Vec<u8>)>,
        verify_files: Vec<(PathBuf, Vec<u8>)>,
        apply_intended: Option<Vec<u8>>,
        verify_intended: Option<Vec<u8>>,
        verify_from: usize,
        launches: Cell<usize>,
        seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    }

    impl Runner for StagedRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let launch = self.launches.get();
            self.launches.set(launch + 1);
            let (files, intended) = if launch >= self.verify_from {
                (&self.verify_files, &self.verify_intended)
            } else {
                (&self.apply_files, &self.apply_intended)
            };
            for (path, bytes) in files {
                std::fs::write(path, bytes).expect("stage artifact");
            }
            if let Some(path) = env
                .iter()
                .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
            {
                if let Some(witness) = intended {
                    std::fs::write(path, witness).expect("stage witness");
                }
            }
            self.seen_env.borrow_mut().push(
                env.iter()
                    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                    .collect(),
            );
            if let Some(bep) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
            {
                std::fs::write(bep, self.bep_lines.join("\n")).expect("BEP file");
            }
            Ok(ChildStatus { code: self.code })
        }
    }

    /// Builds one runner serving `harness` content for the apply pass and
    /// `verify_files`/`verify_intended` from launch `verify_from` on.
    fn staged_runner(
        harness: &Harness,
        verify_files: Vec<(PathBuf, Vec<u8>)>,
        verify_intended: Option<Vec<u8>>,
        verify_from: usize,
    ) -> StagedRunner {
        let staged = harness.staged_artifacts();
        let files: Vec<serde_json::Value> = staged
            .iter()
            .map(|(artifact, _)| {
                serde_json::json!({"uri": format!("file://{}", artifact.to_string_lossy())})
            })
            .collect();
        StagedRunner {
            code: Some(0),
            bep_lines: vec![
                serde_json::json!({
                    "id": {"namedSet": {"id": "0"}},
                    "namedSetOfFiles": {"files": files},
                })
                .to_string(),
                serde_json::json!({
                    "id": {"targetCompleted": {"label": "//test:corpus"}},
                    "completed": {
                        "success": true,
                        "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                    },
                })
                .to_string(),
                LAST_EVENT.to_owned(),
            ],
            apply_files: staged,
            verify_files,
            apply_intended: harness.intended.clone(),
            verify_intended,
            verify_from,
            launches: Cell::new(0),
            seen_env: Rc::clone(&harness.seen_env),
        }
    }

    /// Runs one `fix --apply` whose launches from `verify_from` observe
    /// `verify_files` and `verify_intended` instead of the apply content.
    fn fix_apply_with_staged_verify(
        harness: &Harness,
        verify_files: Vec<(PathBuf, Vec<u8>)>,
        verify_intended: Option<Vec<u8>>,
        verify_from: usize,
        words: &[&str],
    ) -> (i32, String, String) {
        let runner = staged_runner(harness, verify_files, verify_intended, verify_from);
        let inv = umbrella_invocation(harness, words);
        harness.execute_with(&inv, &runner)
    }

    /// Reads one merged SARIF document out of a harness workspace.
    fn merged_runs(harness: &Harness) -> Vec<serde_json::Value> {
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"),
        )
        .expect("SARIF JSON");
        assert_eq!(document["version"], serde_json::json!("2.1.0"));
        assert_eq!(
            document["$schema"],
            serde_json::json!("https://json.schemastore.org/sarif-2.1.0.json")
        );
        document["runs"]
            .as_array()
            .expect("runs")
            .iter()
            .cloned()
            .collect()
    }

    #[test]
    fn check_rejects_build_output_and_names_the_file() {
        let harness = umbrella_clean("umbrella-check-build-output");
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&harness.workspace)
                .args(args)
                .status()
                .expect("git");
            assert!(status.success());
        };
        git(&["init", "-q"]);
        harness.write_source("cli/target/debug/dx", "binary");
        let (code, out, err) = harness.run(&["check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("cli/target/debug/dx"), "{out}{err}");
        assert!(!out.contains("Running format"), "{out}");
        assert_eq!(harness.seen_env.borrow().len(), 0);
    }

    #[test]
    fn check_clean_runs_every_phase_in_order() {
        let harness = umbrella_clean("umbrella-check-clean");
        let (code, out, err) = harness.run(&["check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        let format = out
            .find("Running format analysis for //...")
            .expect("format");
        let lint = out.find("Running lint analysis for //...").expect("lint");
        let typecheck = out
            .find("Running typecheck analysis for //...")
            .expect("typecheck");
        let generate = out.find("Running generate for //...").expect("generate");
        assert!(
            format < lint && lint < typecheck && typecheck < generate,
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            4,
            "one Bazel launch per phase"
        );
    }

    #[test]
    fn check_json_brackets_phase_lifecycles() {
        let harness = umbrella_clean("umbrella-check-json");
        let (code, out, err) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["event"], "command_started");
        assert_eq!(events.first().expect("first")["command"], "check");
        assert_eq!(events.last().expect("last")["event"], "command_finished");
        assert_eq!(events.last().expect("last")["exit_code"], 0);
        assert_eq!(events_of_kind(&events, "command_started").len(), 1);
        assert_eq!(events_of_kind(&events, "command_finished").len(), 1);
        let boundaries: Vec<(&str, &str)> = events
            .iter()
            .filter(|event| event["event"] == "phase_started" || event["event"] == "phase_finished")
            .map(|event| {
                (
                    event["event"].as_str().expect("event"),
                    event["phase"].as_str().expect("phase"),
                )
            })
            .collect();
        assert_eq!(
            boundaries,
            vec![
                ("phase_started", "format"),
                ("phase_finished", "format"),
                ("phase_started", "lint"),
                ("phase_finished", "lint"),
                ("phase_started", "typecheck"),
                ("phase_finished", "typecheck"),
                ("phase_started", "generate"),
                ("phase_finished", "generate"),
            ]
        );
        for event in &events[1..events.len() - 1] {
            assert_eq!(event["command"], "check", "{event}");
            let phase = event["phase"].as_str().expect("phase");
            assert_eq!(
                event["correlation"],
                serde_json::json!(format!("check/{phase}")),
                "{event}"
            );
            assert_eq!(event["schema"]["major"], 3, "{event}");
        }
    }

    #[test]
    fn check_runs_every_phase_after_ordinary_findings() {
        let harness = umbrella_findings("umbrella-continue");
        let (code, out, err) = harness.run(&["check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(out.contains("Running lint analysis for //..."), "{out}");
        assert!(
            out.contains("Running typecheck analysis for //..."),
            "{out}"
        );
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            err.contains(
                "dx: phases: format findings, lint findings, typecheck findings, generate failed"
            ),
            "{err}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "check mode never mutates"
        );
        assert_eq!(harness.seen_env.borrow().len(), 4, "every phase launches");
    }

    #[test]
    fn check_json_reports_every_phase_after_findings() {
        let harness = umbrella_findings("umbrella-continue-json");
        let (code, out, _) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["command"], "check");
        assert_eq!(events.last().expect("last")["event"], "command_finished");
        assert_eq!(events.last().expect("last")["exit_code"], 1);
        assert_eq!(events_of_kind(&events, "command_started").len(), 1);
        assert_eq!(events_of_kind(&events, "command_finished").len(), 1);
        let started: Vec<&str> = events_of_kind(&events, "phase_started")
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect();
        assert_eq!(started, vec!["format", "lint", "typecheck", "generate"]);
        let format_finished = events_of_kind(&events, "phase_finished")
            .into_iter()
            .find(|event| event["phase"] == "format")
            .expect("format finished");
        assert_eq!(format_finished["exit_code"], 1);
        assert_eq!(format_finished["correlation"], "check/format");
        let findings: Vec<&str> = events_of_kind(&events, "notice")
            .iter()
            .filter(|notice| notice["code"] == "umbrella_phase_findings")
            .map(|notice| notice["correlation"].as_str().expect("correlation"))
            .collect();
        assert_eq!(
            findings,
            vec!["check/format", "check/lint", "check/typecheck"]
        );
    }

    #[test]
    fn fix_check_flag_forces_check_mode() {
        let harness = umbrella_findings("umbrella-fix-check");
        let (code, out, _) = harness.run(&["fix", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "forced check mode never mutates"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            4,
            "every phase runs after findings"
        );
    }

    #[test]
    fn fix_applies_generate_mutation_after_clean_quality() {
        let mut harness = Harness::new("umbrella-fix");
        harness.write_source("src/a.py", "x = 1\n");
        harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness.intended = Some(intended_witness(
            "default",
            true,
            &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
            "",
        ));
        let staged = harness.staged_artifacts();
        let (code, out, err) = fix_apply_with_staged_verify(
            &harness,
            staged,
            Some(intended_witness("check", true, "", "")),
            4,
            &["fix", "--apply", "--output=text"],
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            8,
            "four apply launches plus four verification launches"
        );
    }

    #[test]
    fn fix_default_checks_without_writing() {
        let harness = umbrella_findings("umbrella-fix-default");
        let (code, out, err) = harness.run(&["fix", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(out.contains("Running lint analysis"), "{out}");
        assert!(out.contains("Running typecheck analysis"), "{out}");
        assert!(out.contains("Running generate"), "{out}");
        assert!(
            err.contains("dx: phases: format findings, lint findings, typecheck findings"),
            "{err}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "default fix never mutates"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            4,
            "every phase runs after findings"
        );
    }

    #[test]
    fn fix_apply_verifies_quality_apply_with_clean_recheck() {
        let mut harness = Harness::new("umbrella-fix-verify");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        harness.intended = Some(intended_witness("default", true, "", ""));
        let staged = harness.staged_artifacts();
        let clean = harness.result_full(vec![], vec![], vec![], vec![]);
        let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
            .iter()
            .map(|(path, _)| (path.clone(), clean.clone()))
            .collect();
        let (code, out, err) = fix_apply_with_staged_verify(
            &harness,
            verify_files,
            Some(intended_witness("check", true, "", "")),
            4,
            &["fix", "--apply", "--output=text"],
        );
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "the apply pass wrote the validated edit"
        );
        assert!(out.contains("Applied 1 file(s)."), "{out}");
        assert!(
            err.contains("Not applied: src/a.py (stale_source)"),
            "later phases name the edit the first apply left stale: {err}"
        );
        assert_eq!(
            out.matches("Running format analysis for //...").count(),
            1,
            "a clean verification stays silent: {out}"
        );
        assert_eq!(
            out.matches("Running generate").count(),
            1,
            "generate touched nothing, so verification stops at typecheck: {out}"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            7,
            "four apply launches plus format, lint and typecheck verification"
        );
    }

    #[test]
    fn fix_apply_clean_runs_no_verification() {
        let mut harness = umbrella_clean("umbrella-fix-clean-no-verify");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["fix", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out.matches("Running format analysis for //...").count(),
            1,
            "{out}"
        );
        assert_eq!(out.matches("Running generate").count(), 1, "{out}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            4,
            "four apply launches and no verification"
        );
    }

    #[test]
    fn fix_apply_json_clean_emits_no_verify_events() {
        let mut harness = umbrella_clean("umbrella-fix-clean-no-verify-json");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["fix", "--apply", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert!(
            events.iter().all(|event| !event["correlation"]
                .as_str()
                .is_some_and(|correlation| correlation.starts_with("verify/"))),
            "{out}"
        );
        assert_eq!(
            events.last().expect("last")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(harness.seen_env.borrow().len(), 4);
    }

    #[test]
    fn fix_apply_fails_when_verification_finds_drift() {
        let mut harness = Harness::new("umbrella-fix-verify-fails");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        harness.intended = Some(intended_witness("default", true, "", ""));
        let staged = harness.staged_artifacts();
        let drift = harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        );
        let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
            .iter()
            .map(|(path, _)| (path.clone(), drift.clone()))
            .collect();
        let (code, out, err) = fix_apply_with_staged_verify(
            &harness,
            verify_files,
            Some(intended_witness("check", true, "", "")),
            4,
            &["fix", "--apply", "--output=text"],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "only the apply pass writes; verification never does"
        );
        assert!(
            err.contains("dx: verification_failed: format failed verification after apply"),
            "{err}"
        );
        assert!(
            err.contains("verification skipped for lint, typecheck\n"),
            "{err}"
        );
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert_eq!(
            out.matches("Running format analysis for //...").count(),
            2,
            "apply ran format, then verification re-ran it: {out}"
        );
        assert_eq!(
            out.matches("Running lint analysis for //...").count(),
            1,
            "verification stops after the first failing phase: {out}"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            5,
            "four apply launches plus one verification launch"
        );
    }

    #[test]
    fn fix_apply_json_reports_verification_failure() {
        let mut harness = Harness::new("umbrella-fix-verify-json");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        harness.intended = Some(intended_witness("default", true, "", ""));
        let staged = harness.staged_artifacts();
        let drift = harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        );
        let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
            .iter()
            .map(|(path, _)| (path.clone(), drift.clone()))
            .collect();
        let (code, out, _) = fix_apply_with_staged_verify(
            &harness,
            verify_files,
            Some(intended_witness("check", true, "", "")),
            4,
            &["fix", "--apply", "--output=json"],
        );
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["command"], "fix");
        assert_eq!(events.first().expect("first")["mode"], "default");
        let failure = event(&events, "error");
        assert_eq!(failure["code"], serde_json::json!("verification_failed"));
        assert_eq!(failure["phase"], serde_json::json!("format"));
        assert_eq!(failure["correlation"], serde_json::json!("fix/format"));
        assert_eq!(
            events.last().expect("last")["exit_code"],
            serde_json::json!(1)
        );
    }

    fn oscillating(harness: &Harness) -> Vec<u8> {
        let mut result = quality_result::decode_validated(&harness.valid_result(vec![], vec![]))
            .expect("valid result");
        result.convergence = quality_result::proto::Convergence::Oscillation as i32;
        result.completed_rounds = 2;
        quality_result::encode_validated(&result).expect("encode")
    }

    const NOT_CONVERGED: &str = "dx: not_converged: //test:corpus did not converge: \
                                 OSCILLATION after 2 round(s) of lint-tool";

    #[test]
    fn check_and_fix_keep_going_after_a_diagnostic_free_unstable_phase() {
        for words in [
            ["check", "--output=text"],
            ["fix", "--output=text"],
            ["fix", "--apply"],
        ] {
            let mut harness = Harness::new("umbrella-unstable");
            harness.write_source("src/a.py", "x = 1\n");
            let bytes = oscillating(&harness);
            harness.results.insert("//test:corpus".to_owned(), bytes);
            let (code, out, err) = harness.run(&words);
            assert_eq!(code, 1, "{words:?}: {out}{err}");
            assert!(err.contains(NOT_CONVERGED), "{words:?}: {err}");
            assert!(out.contains("Running lint analysis"), "{words:?}: {out}");
            assert!(out.contains("Running generate"), "{words:?}: {out}");
            assert_eq!(
                std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
                b"x = 1\n",
                "{words:?} keeps the source bytes"
            );
            assert_eq!(harness.seen_env.borrow().len(), 4, "{words:?}");
        }
    }

    #[test]
    fn check_json_forwards_the_unstable_producer() {
        let mut harness = Harness::new("umbrella-unstable-json");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = oscillating(&harness);
        harness.results.insert("//test:corpus".to_owned(), bytes);
        let (code, out, _) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        let failure = events
            .iter()
            .find(|event| event["event"] == "error")
            .expect("error event");
        assert_eq!(failure["code"], "not_converged");
        assert_eq!(
            failure["message"],
            "//test:corpus did not converge: OSCILLATION after 2 round(s) of lint-tool"
        );
        assert_eq!(events.last().expect("last")["exit_code"], 1);
    }

    #[test]
    fn fix_apply_verification_fails_on_an_unstable_recheck() {
        let mut harness = Harness::new("umbrella-fix-verify-unstable");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        harness.intended = Some(intended_witness("default", true, "", ""));
        let staged = harness.staged_artifacts();
        let unstable =
            quality_result::decode_validated(&harness.result_full(vec![], vec![], vec![], vec![]))
                .map(|mut result| {
                    result.convergence = quality_result::proto::Convergence::Oscillation as i32;
                    result.completed_rounds = 2;
                    quality_result::encode_validated(&result).expect("encode")
                })
                .expect("valid result");
        let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
            .iter()
            .map(|(path, _)| (path.clone(), unstable.clone()))
            .collect();
        let (code, out, err) = fix_apply_with_staged_verify(
            &harness,
            verify_files,
            Some(intended_witness("check", true, "", "")),
            4,
            &["fix", "--apply", "--output=text"],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "the stable apply pass wrote its edit"
        );
        assert!(err.contains(NOT_CONVERGED), "{err}");
        assert!(
            err.contains("dx: verification_failed: format failed verification after apply"),
            "{err}"
        );
        assert!(
            err.contains("verification skipped for lint, typecheck\n"),
            "{err}"
        );
    }

    #[test]
    fn fix_keeps_going_when_a_later_phase_goes_stale() {
        let harness = umbrella_findings("umbrella-stale");
        let (code, out, _) = harness.run(&["fix", "--apply", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(out.contains("Running lint analysis for //..."), "{out}");
        assert!(out.contains("Running typecheck analysis"), "{out}");
        assert!(out.contains("Running generate"), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "format applied before lint went stale"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            4,
            "every phase launches once and no verification runs"
        );
        assert!(
            harness
                .seen_env
                .borrow()
                .iter()
                .flatten()
                .any(|(key, _)| key == GENERATE_ENV_INTENDED),
            "generate dispatch ran"
        );
    }

    #[test]
    fn fix_runs_every_phase_after_a_tool_failure() {
        let harness = umbrella_clean("umbrella-fix-tool-failure");
        let runner = FailFrom {
            inner: harness.runner(),
            from: 0,
            to: Some(1),
            code: 1,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(&harness, &["fix", "--output=json"]);
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(runner.launches.get(), 4, "every phase launches");
        let events = json_events(&out);
        let started: Vec<&str> = events_of_kind(&events, "phase_started")
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect();
        assert_eq!(started, vec!["format", "lint", "typecheck", "generate"]);
        let failures: Vec<&str> = events_of_kind(&events, "error")
            .iter()
            .map(|event| event["correlation"].as_str().expect("correlation"))
            .collect();
        assert_eq!(failures, vec!["fix/format"], "{out}");
        assert!(
            events_of_kind(&events, "notice")
                .iter()
                .all(|notice| notice["code"] != "umbrella_phase_skipped"),
            "{out}"
        );
        let finished = *events_of_kind(&events, "command_finished")
            .last()
            .expect("umbrella finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
    }

    #[test]
    fn fix_apply_skips_verification_after_a_failed_phase() {
        let mut harness = Harness::new("umbrella-fix-failed-no-verify");
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![harness.replacement(b"y")]),
        );
        harness.intended = Some(intended_witness("default", true, "", ""));
        let runner = FailFrom {
            inner: harness.runner(),
            from: 2,
            to: Some(3),
            code: 1,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(&harness, &["fix", "--apply", "--output=json"]);
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "format applied before typecheck failed"
        );
        assert_eq!(runner.launches.get(), 4, "only the four phase launches");
        let events = json_events(&out);
        assert!(
            events.iter().all(|event| !event["correlation"]
                .as_str()
                .is_some_and(|correlation| correlation.starts_with("verify/"))),
            "{out}"
        );
        assert_eq!(
            events.last().expect("last")["exit_code"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn check_runs_every_phase_after_a_tool_failure() {
        let harness = umbrella_clean("umbrella-check-tool-failure");
        let runner = FailFrom {
            inner: harness.runner(),
            from: 1,
            to: Some(2),
            code: 1,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(&harness, &["check", "--output=text"]);
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            out.contains("Running typecheck analysis for //..."),
            "{out}"
        );
        assert!(out.contains("Running generate for //..."), "{out}");
        assert!(
            err.contains("dx: analysis_failed: lint analysis failed"),
            "{err}"
        );
        assert!(
            err.contains("dx: phases: format clean, lint failed, typecheck clean, generate clean"),
            "{err}"
        );
        assert_eq!(runner.launches.get(), 4, "every phase launches");
    }

    #[test]
    fn umbrella_stdout_report_is_rejected_pre_exec() {
        let harness = umbrella_clean("umbrella-stdout-report");
        let (code, _, err) = harness.run(&["check", "--report=sarif=-", "--output=text"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("share one stdout document"), "{err}");
        assert!(harness.seen_env.borrow().is_empty(), "no phase launched");
    }

    #[test]
    fn umbrella_unknown_report_is_rejected_pre_exec() {
        let harness = umbrella_clean("umbrella-unknown-report");
        let (code, _, err) = harness.run(&["check", "--report=junit=out.xml"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("unsupported report format"), "{err}");
        assert!(harness.seen_env.borrow().is_empty(), "no phase launched");
    }

    #[test]
    fn umbrella_sarif_merges_executed_phases_in_order() {
        let harness = umbrella_clean("umbrella-sarif");
        let (code, out, err) = harness.run(&["check", "--output=json", "--report=sarif=out.sarif"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let report = *events_of_kind(&events, "report")
            .last()
            .expect("umbrella report");
        assert_eq!(report["format"], serde_json::json!("sarif"));
        assert_eq!(report["path"], serde_json::json!("out.sarif"));
        assert_eq!(
            report["results_complete"],
            serde_json::json!(true),
            "every phase ran and every capture parsed"
        );
        assert_eq!(
            event(&events, "command_finished")["results_complete"],
            serde_json::json!(true)
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"),
        )
        .expect("SARIF JSON");
        assert_eq!(
            document["runs"].as_array().expect("runs").len(),
            2,
            "lint and typecheck each contribute one run"
        );
        assert_eq!(
            document["runs"][0], document["runs"][1],
            "both phases replay the shared clean fixture"
        );

        let (code, out, _) = harness.run(&["check", "--output=text", "--report=sarif=out.sarif"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            out.lines().last().expect("last line"),
            "Wrote sarif report to out.sarif."
        );
    }

    #[test]
    fn fix_sarif_covers_only_executed_phases() {
        let mut harness = umbrella_findings("umbrella-sarif-stop");
        harness.free_disk_bytes = Some(1 << 20);
        let run = |output: &str| {
            let runner = FailFrom {
                inner: harness.runner(),
                from: 1,
                to: None,
                code: 36,
                launches: Cell::new(0),
            };
            let inv = umbrella_invocation(
                &harness,
                &["fix", "--check", output, "--report=sarif=out.sarif"],
            );
            harness.execute_with(&inv, &runner)
        };
        let (code, out, err) = run("--output=json");
        assert_eq!(code, 36, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(
            events_of_kind(&events, "report")
                .last()
                .expect("umbrella report")["results_complete"],
            serde_json::json!(false),
            "a run stopped at lint leaves the report incomplete"
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"),
        )
        .expect("SARIF JSON");
        assert_eq!(
            document["runs"].as_array().expect("runs").len(),
            0,
            "no SARIF-capable phase finished, no runs"
        );

        let (code, _, err) = run("--output=diff");
        assert_eq!(code, 36);
        assert!(
            err.ends_with(
                "Wrote sarif report to out.sarif (incomplete: lint failed, \
                 typecheck skipped, generate skipped).\n\
                 dx: phases: format findings, lint failed, typecheck skipped, generate skipped\n"
            ),
            "{err}"
        );
    }

    #[test]
    fn umbrella_sarif_skips_phase_that_could_not_render() {
        let harness = umbrella_findings("umbrella-sarif-unrendered");
        let (code, out, err) = harness.run(&[
            "fix",
            "--apply",
            "--output=text",
            "--report=sarif=out.sarif",
        ]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to render SARIF report"), "{err}");
        assert!(
            out.contains(
                "Wrote sarif report to out.sarif \
                 (incomplete: lint failed, typecheck failed, generate failed)."
            ),
            "{out}"
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"),
        )
        .expect("SARIF JSON");
        assert!(
            document["runs"].as_array().expect("runs").is_empty(),
            "the phase that never rendered contributes no runs"
        );
    }

    #[test]
    fn umbrella_report_write_failure_fails() {
        let harness = umbrella_clean("umbrella-report-fail");
        let (code, _, err) = harness.run(&[
            "check",
            "--output=text",
            "--report=sarif=missing-dir/out.sarif",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("report_failed"), "{err}");

        let (code, out, _) = harness.run(&[
            "check",
            "--output=json",
            "--report=sarif=missing-dir/out.sarif",
        ]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("\"code\":\"report_failed\""), "{out}");
    }

    #[test]
    fn umbrella_failed_phase_and_failed_report_still_fails() {
        let harness = umbrella_findings("umbrella-report-fail-stop");
        let (code, _, err) = harness.run(&[
            "check",
            "--output=text",
            "--report=sarif=missing-dir/out.sarif",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("report_failed"), "{err}");
    }

    #[test]
    fn fix_json_reports_umbrella_lifecycle() {
        let harness = umbrella_clean("umbrella-fix-json");
        let (code, out, err) = harness.run(&["fix", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["command"], "fix");
        assert_eq!(
            events.first().expect("first")["mode"],
            "check",
            "bare fix checks without --apply"
        );
        assert_eq!(events.last().expect("last")["exit_code"], 0);
    }

    #[test]
    fn umbrella_diff_renders_every_phase_patch() {
        let harness = umbrella_findings("umbrella-diff");
        let (code, out, _) = harness.run(&["check", "--output=diff"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("src/a.py"), "{out}");
        assert_eq!(harness.seen_env.borrow().len(), 4, "every phase launches");
    }

    #[test]
    fn check_json_correlates_one_phase_start_per_phase() {
        let harness = umbrella_clean("umbrella-phase-events");
        let (code, out, err) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert!(events_of_kind(&events, "operation").is_empty());
        let started = events_of_kind(&events, "phase_started");
        let described: Vec<(&str, &str, &str)> = started
            .iter()
            .map(|event| {
                (
                    event["command"].as_str().expect("command"),
                    event["phase"].as_str().expect("phase"),
                    event["mode"].as_str().expect("mode"),
                )
            })
            .collect();
        assert_eq!(
            described,
            vec![
                ("check", "format", "check"),
                ("check", "lint", "check"),
                ("check", "typecheck", "check"),
                ("check", "generate", "check")
            ]
        );
        for (index, phase) in ["format", "lint", "typecheck", "generate"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                started[index]["correlation"],
                serde_json::json!(format!("check/{phase}")),
                "{phase}"
            );
        }
        assert!(
            events_of_kind(&events, "notice").is_empty(),
            "nothing skipped, nothing to notice"
        );
    }

    #[test]
    fn fix_reports_a_full_disk_once_and_skips_the_later_phases() {
        let mut harness = umbrella_clean("umbrella-disk-full");
        harness.free_disk_bytes = Some(1 << 20);
        let inv = umbrella_invocation(&harness, &["fix", "--output=json"]);
        let runner = FailFrom {
            inner: harness.runner(),
            from: 1,
            to: None,
            code: 36,
            launches: Cell::new(0),
        };
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 36, "{out}{err}");
        assert!(err.contains("dx: disk_full: "), "{err}");
        assert!(!err.contains("analysis_failed"), "{err}");
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], serde_json::json!("disk_full"));
        assert_eq!(errors[0]["cause"], serde_json::json!("host"));
        assert_eq!(errors[0]["correlation"], serde_json::json!("fix/lint"));
        let skipped: Vec<&str> = events_of_kind(&events, "notice")
            .iter()
            .filter(|notice| notice["code"] == serde_json::json!("umbrella_phase_skipped"))
            .filter_map(|notice| notice["message"].as_str())
            .collect();
        assert_eq!(
            skipped,
            vec![
                "typecheck skipped after lint failed to run its tools",
                "generate skipped after lint failed to run its tools",
            ]
        );
    }

    #[test]
    fn check_json_names_every_skipped_phase() {
        let mut harness = umbrella_clean("umbrella-skip-events");
        harness.free_disk_bytes = Some(1 << 20);
        let runner = FailFrom {
            inner: harness.runner(),
            from: 1,
            to: None,
            code: 36,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(
            &harness,
            &["check", "--output=json", "--report=sarif=out.sarif"],
        );
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 36, "{out}{err}");
        let events = json_events(&out);
        let skipped: Vec<(&str, &str)> = events_of_kind(&events, "notice")
            .iter()
            .filter(|notice| notice["code"] != "disk_low")
            .map(|notice| {
                (
                    notice["code"].as_str().expect("code"),
                    notice["message"].as_str().expect("message"),
                )
            })
            .collect();
        assert_eq!(
            skipped,
            vec![
                (
                    "umbrella_phase_skipped",
                    "typecheck skipped after lint failed to run its tools"
                ),
                (
                    "umbrella_phase_skipped",
                    "generate skipped after lint failed to run its tools"
                ),
            ]
        );
        for notice in events_of_kind(&events, "notice")
            .into_iter()
            .filter(|notice| notice["code"] == "umbrella_phase_skipped")
        {
            assert_eq!(notice["related_command"], serde_json::json!("check"));
            assert!(notice["correlation"].is_string(), "{notice}");
        }
        assert_eq!(
            events_of_kind(&events, "report")
                .last()
                .expect("umbrella report")["results_complete"],
            serde_json::json!(false)
        );
        let finished = *events_of_kind(&events, "command_finished")
            .last()
            .expect("umbrella finished");
        assert_eq!(finished["exit_code"], serde_json::json!(36));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
        assert_eq!(
            harness.seen_env.borrow().len(),
            2,
            "unsafe work never starts"
        );
    }

    #[test]
    fn check_missing_capture_fails_the_run_and_keeps_valid_partial_runs() {
        let (harness, code, out, err) = umbrella_with_broken_capture(
            umbrella_clean("umbrella-missing-capture"),
            Rewrite::Remove,
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            err, "dx: collection_failed: lint wrote no sarif capture\n",
            "{err}"
        );
        assert_eq!(
            out.lines().last().expect("last line"),
            "Wrote sarif report to out.sarif (incomplete: lint wrote no sarif capture)."
        );
        for phase in ["format", "lint", "typecheck", "generate"] {
            let marker = match phase {
                "generate" => "Running generate for //...",
                _ => &format!("Running {phase} analysis for //..."),
            };
            assert!(out.contains(marker), "{phase} ran: {out}");
        }
        let runs = merged_runs(&harness);
        assert_eq!(
            runs.len(),
            1,
            "the typecheck capture still lands in the merged document"
        );
        assert!(
            !capture_path(&harness, "lint", 1).exists(),
            "the capture is deleted even when it was never written"
        );
    }

    #[test]
    fn check_missing_capture_reports_an_incomplete_json_document() {
        let (harness, code, out, err) = umbrella_with_broken_capture(
            umbrella_clean("umbrella-missing-capture-json"),
            Rewrite::Remove,
            &["check", "--output=json", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let failure = event(&events, "error");
        assert_eq!(failure["code"], serde_json::json!(CODE_COLLECTION_FAILED));
        assert_eq!(
            failure["message"],
            serde_json::json!("lint wrote no sarif capture")
        );
        assert_eq!(failure["phase"], serde_json::json!("lint"));
        assert_eq!(failure["correlation"], serde_json::json!("check/lint"));
        assert_eq!(
            events_of_kind(&events, "report")
                .last()
                .expect("umbrella report")["results_complete"],
            serde_json::json!(false)
        );
        let finished = *events_of_kind(&events, "command_finished")
            .last()
            .expect("umbrella finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
        assert_eq!(merged_runs(&harness).len(), 1, "{out}");
    }

    #[test]
    fn check_unreadable_capture_fails_the_run() {
        let (harness, code, out, err) = umbrella_with_broken_capture(
            umbrella_clean("umbrella-unreadable-capture"),
            Rewrite::Block,
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            err, "dx: collection_failed: lint sarif capture is unreadable\n",
            "{err}"
        );
        assert!(
            out.contains("(incomplete: lint sarif capture is unreadable)"),
            "{out}"
        );
        assert_eq!(merged_runs(&harness).len(), 1, "{out}");
    }

    #[test]
    fn fix_apply_mode_shares_the_same_capture_collector() {
        let mut harness = umbrella_clean("umbrella-fix-missing-capture");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let staged = harness.staged_artifacts();
        let runner = staged_runner(
            &harness,
            staged,
            Some(intended_witness("check", true, "", "")),
            4,
        );
        let capture = capture_path(&harness, "lint", 1);
        let breaking = RewriteCapture {
            inner: runner,
            capture,
            rewrite: Rewrite::Remove,
            done: Cell::new(false),
        };
        let inv = umbrella_invocation(
            &harness,
            &[
                "fix",
                "--apply",
                "--output=text",
                "--report=sarif=out.sarif",
            ],
        );
        let (code, out, err) = harness.execute_with(&inv, &breaking);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(
            err, "dx: collection_failed: lint wrote no sarif capture\n",
            "{err}"
        );
        assert!(
            out.contains("Running generate for //..."),
            "apply mode still runs every phase: {out}"
        );
        assert_eq!(
            out.lines().last().expect("last line"),
            "Wrote sarif report to out.sarif (incomplete: lint wrote no sarif capture)."
        );
        assert_eq!(merged_runs(&harness).len(), 1, "{out}");
    }

    #[test]
    fn check_malformed_capture_fails_the_run() {
        for (name, payload) in [
            ("not json at all", &b"{not json"[..]),
            ("an empty document", &b"{}"[..]),
            ("a document without runs", &br#"{"version":"2.1.0"}"#[..]),
            (
                "a document whose runs are not an array",
                &br#"{"version":"2.1.0","runs":{}}"#[..],
            ),
            (
                "a run without a tool",
                &br#"{"version":"2.1.0","runs":[{"nope":1}]}"#[..],
            ),
            (
                "a tool without a driver",
                &br#"{"version":"2.1.0","runs":[{"tool":{}}]}"#[..],
            ),
            (
                "a driver without a name",
                &br#"{"version":"2.1.0","runs":[{"tool":{"driver":{}}}]}"#[..],
            ),
            ("a bare JSON array", &b"[]"[..]),
        ] {
            let (harness, code, out, err) = umbrella_with_broken_capture(
                umbrella_clean(&format!("umbrella-malformed-{name}")),
                Rewrite::Bytes(payload),
                &["check", "--output=text", "--report=sarif=out.sarif"],
            );
            assert_eq!(code, 1, "{name}: {out}{err}");
            assert_eq!(
                err, "dx: collection_failed: lint sarif capture is malformed\n",
                "{name}: {err}"
            );
            assert_eq!(merged_runs(&harness).len(), 1, "{name}: {out}");
        }
    }

    #[test]
    fn check_merged_document_is_deterministic() {
        let (harness, code, _, err) = umbrella_with_broken_capture(
            umbrella_clean("umbrella-deterministic"),
            Rewrite::Bytes(br#"{"version":"2.1.0","runs":[{"nope":1}]}"#),
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{err}");
        let first = std::fs::read(harness.workspace.join("out.sarif")).expect("first");
        let (code, _, err) = harness.run(&["check", "--output=text", "--report=sarif=out.sarif"]);
        assert_eq!(code, 0, "{err}");
        let complete = std::fs::read(harness.workspace.join("out.sarif")).expect("second");
        assert_eq!(merged_runs(&harness).len(), 2);
        assert_ne!(first, complete, "the broken run never reaches the document");
    }

    #[test]
    fn umbrella_broken_pipe_stdout_fails_with_141() {
        struct BrokenPipe;
        impl Write for BrokenPipe {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
            }
        }

        let harness = umbrella_clean("umbrella-broken-pipe");
        let inv = umbrella_invocation(
            &harness,
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        let runner = harness.runner();
        let mut out = BrokenPipe;
        let mut err = Vec::new();
        assert_eq!(harness.run_streams(&inv, &runner, &mut out, &mut err), 141);
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "a broken phase stream stops the run after the phase that filled it"
        );
        assert!(
            !harness.workspace.join("out.sarif").exists(),
            "no report is announced or written after stdout breaks"
        );
    }

    #[test]
    fn umbrella_stdout_write_failure_is_operational() {
        struct FailAfterOne {
            seen: usize,
        }
        impl Write for FailAfterOne {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.seen += 1;
                if self.seen == 1 {
                    return Ok(buf.len());
                }
                Err(io::Error::other("boom"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let harness = umbrella_clean("umbrella-stdout-failure");
        let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
        let runner = harness.runner();
        let mut out = FailAfterOne { seen: 0 };
        let mut err = Vec::new();
        assert_eq!(
            harness.run_streams(&inv, &runner, &mut out, &mut err),
            1,
            "the first event lands, the phase events do not"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "no later phase launched after stdout failed"
        );
        assert_eq!(out.seen, 2, "nothing is written after the failed write");
        assert!(
            err.is_empty(),
            "a stdout failure is not a stderr diagnostic"
        );
    }

    #[test]
    fn collector_classifies_every_capture_state() {
        let dir = temp_dir("umbrella-collector");
        let good = dir.path().join("good.sarif");
        let bad = dir.path().join("bad.sarif");
        let missing = dir.path().join("missing.sarif");
        std::fs::write(
            &good,
            crate::reports::render_sarif(
                &["lint-tool".to_owned()],
                &[],
                &std::collections::BTreeMap::new(),
                true,
            )
            .expect("render"),
        )
        .expect("good capture");
        std::fs::write(&bad, b"{}").expect("bad capture");

        let mut collector = PhaseCollector::default();
        for phase in UMBRELLA_PHASES {
            collector.plan(phase);
        }
        collector.require_capture(0, good.clone());
        collector.record_exit(0, 0, PhaseKind::Clean);
        collector.require_capture(1, bad.clone());
        collector.record_exit(1, 0, PhaseKind::Clean);
        collector.require_capture(2, missing.clone());
        collector.record_exit(2, 0, PhaseKind::Clean);
        collector.record_exit(3, 1, PhaseKind::Tool);
        collector.skip_remaining(String::new());
        collector.collect();

        assert_eq!(
            collector.collection_failures(),
            vec![
                ("lint", "sarif capture is malformed"),
                ("typecheck", "wrote no sarif capture"),
            ]
        );
        assert_eq!(
            collector.reasons(),
            vec![
                "lint sarif capture is malformed".to_owned(),
                "typecheck wrote no sarif capture".to_owned(),
                "generate failed".to_owned(),
            ]
        );
        assert!(!collector.complete());
        let document: serde_json::Value =
            serde_json::from_str(&collector.document()).expect("JSON");
        assert_eq!(document["runs"].as_array().expect("runs").len(), 1);

        for path in [&good, &bad, &missing] {
            collector.cleanup();
            assert!(!path.exists(), "{path:?} deleted");
        }
    }

    #[test]
    fn collector_merges_a_complete_run_deterministically() {
        let dir = temp_dir("umbrella-collector-complete");
        let captures: Vec<PathBuf> = ["lint", "typecheck"]
            .iter()
            .map(|name| {
                let path = dir.path().join(format!("{name}.sarif"));
                std::fs::write(
                    &path,
                    crate::reports::render_sarif(
                        &[format!("{name}-tool")],
                        &[],
                        &std::collections::BTreeMap::new(),
                        true,
                    )
                    .expect("render"),
                )
                .expect("capture");
                path
            })
            .collect();

        let mut collector = PhaseCollector::default();
        for (index, phase) in UMBRELLA_PHASES.iter().enumerate() {
            collector.plan(*phase);
            if let Some(path) = captures.get(index) {
                collector.require_capture(index, path.clone());
            }
            collector.record_exit(index, 0, PhaseKind::Clean);
        }
        collector.collect();
        assert!(collector.complete(), "{:?}", collector.reasons());
        assert!(collector.collection_failures().is_empty());
        assert!(collector.skipped().is_empty());
        assert_eq!(collector.reasons(), Vec::<String>::new());
        let document = collector.document();
        assert_eq!(document, collector.document(), "same input, same bytes");
        let parsed: serde_json::Value = serde_json::from_str(&document).expect("JSON");
        let runs = parsed["runs"].as_array().expect("runs");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0]["tool"]["driver"]["name"], "lint-tool");
        assert_eq!(runs[1]["tool"]["driver"]["name"], "typecheck-tool");
    }

    #[test]
    fn test_phase_kind_maps_each_bazel_exit() {
        let cases = [
            (0, true, false, PhaseKind::Clean),
            (3, true, false, PhaseKind::Findings),
            (3, false, false, PhaseKind::Incomplete),
            (1, false, false, PhaseKind::Incomplete),
            (1, true, false, PhaseKind::Incomplete),
            (2, false, false, PhaseKind::Selection),
            (4, true, false, PhaseKind::Selection),
            (8, false, false, PhaseKind::Cancelled),
            (36, false, false, PhaseKind::Tool),
            (141, true, false, PhaseKind::Tool),
            (1, false, true, PhaseKind::Host),
            (1, true, true, PhaseKind::Host),
            (0, true, true, PhaseKind::Clean),
        ];
        for (code, complete, launch_failed, kind) in cases {
            assert_eq!(
                test_phase_kind(code, complete, launch_failed),
                kind,
                "{code} {complete} {launch_failed}"
            );
        }
    }

    #[test]
    fn test_phase_results_complete_follows_kind() {
        let collect = |code: i32, kind: PhaseKind| {
            let mut collector = PhaseCollector::default();
            for phase in UMBRELLA_PHASES {
                collector.plan(phase);
            }
            collector.plan(Command::Test);
            for index in 0..UMBRELLA_PHASES.len() {
                collector.record_exit(index, 0, PhaseKind::Clean);
            }
            collector.record_exit(UMBRELLA_PHASES.len(), code, kind);
            collector.skip_remaining(String::new());
            collector.collect();
            collector
        };
        let findings = collect(3, PhaseKind::Findings);
        assert!(findings.complete(), "{:?}", findings.reasons());
        assert!(findings.reasons().is_empty());
        assert_eq!(findings.with_findings(), vec!["test"]);
        for code in [1, 3] {
            let incomplete = collect(code, PhaseKind::Incomplete);
            assert!(!incomplete.complete());
            assert_eq!(incomplete.reasons(), vec!["test failed".to_owned()]);
            assert!(incomplete.with_findings().is_empty());
        }
    }

    /// Runs the umbrella test phase against a harness and returns its outcome.
    fn run_test_phase(harness: &Harness) -> (i32, PhaseKind) {
        let inv = umbrella_invocation(harness, &["test", "--output=text"]);
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let run = execute_test_phase(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(run.effects, PhaseEffects::default());
        (run.code, run.kind)
    }

    #[test]
    fn test_phase_reads_dx_test_completeness() {
        const FAILING_XML: &str = r#"<testsuites><testsuite name="s"><testcase name="fails" classname="c"><failure message="boom"/></testcase></testsuite></testsuites>"#;
        let failed_line = |uri: String| {
            test_result_full_line(
                "//a:t",
                1,
                1,
                1,
                "FAILED",
                None,
                None,
                None,
                &[(String::from("test.xml"), uri)],
            )
        };
        let harness = Harness::new("umbrella-test-phase-findings");
        let uri = write_bep_artifact(&harness, "test.xml", FAILING_XML.as_bytes());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![failed_line(uri)]),
            ..harness
        };
        assert_eq!(run_test_phase(&harness), (3, PhaseKind::Findings));

        let harness = Harness::new("umbrella-test-phase-missing");
        let missing = format!("file://{}", harness.temp.join("absent.xml").display());
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(vec![failed_line(missing)]),
            ..harness
        };
        assert_eq!(run_test_phase(&harness), (3, PhaseKind::Incomplete));

        let harness = Harness {
            bazel_code: 1,
            ..Harness::new("umbrella-test-phase-build-failed")
        };
        assert_eq!(run_test_phase(&harness), (1, PhaseKind::Incomplete));
    }

    #[test]
    fn test_phase_reports_launch_failure_as_host() {
        let harness = Harness {
            io_error: true,
            ..Harness::new("umbrella-test-phase-launch")
        };
        assert_eq!(run_test_phase(&harness), (1, PhaseKind::Host));

        let harness = Harness {
            signalled: true,
            ..Harness::new("umbrella-test-phase-signalled")
        };
        assert_eq!(run_test_phase(&harness), (1, PhaseKind::Host));

        let harness = Harness {
            bazel_code: 1,
            ..Harness::new("umbrella-test-phase-launch-build-failed")
        };
        assert_eq!(run_test_phase(&harness), (1, PhaseKind::Incomplete));
    }

    #[test]
    fn test_phase_launch_watch_forwards_the_inner_runner() {
        let harness = Harness {
            raw_bep: Some(Vec::new()),
            ..Harness::new("umbrella-test-phase-watch")
        };
        let mut runner = harness.runner();
        runner.free_disk_bytes = Some(4096);
        let argv = vec!["bazel".to_owned()];
        let watch = LaunchWatch::new(&runner);
        assert_eq!(watch.free_disk_bytes(&harness.temp), Some(4096));
        assert!(watch.run(&argv, &harness.temp, &[]).is_ok());
        assert!(!watch.failed());

        runner.io_error = true;
        let watch = LaunchWatch::new(&runner);
        assert!(watch.run(&argv, &harness.temp, &[]).is_err());
        assert!(watch.failed());
    }

    /// Runs `check` where format passes and lint reports findings.
    fn check_with_failing_lint(name: &str, words: &[&str]) -> (Harness, i32, String, String) {
        let harness = umbrella_clean(name);
        let (artifact, _) = harness
            .staged_artifacts()
            .into_iter()
            .next()
            .expect("artifact");
        let findings = harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        );
        let staged = staged_runner(
            &harness,
            vec![(artifact, findings)],
            harness.intended.clone(),
            1,
        );
        let inv = umbrella_invocation(&harness, words);
        let (code, out, err) = harness.execute_with(&inv, &staged);
        (harness, code, out, err)
    }

    #[test]
    fn check_affected_hints_name_dx_fix_with_the_affected_flag() {
        let harness = umbrella_findings("umbrella-affected-hint");
        let mut inv = umbrella_invocation(&harness, &["check", "--affected", "--output=text"]);
        inv.affected = false;
        inv.targets = vec!["//test:corpus".to_owned(), "//test:other".to_owned()];
        inv.test_targets = Some(Vec::new());
        let (code, out, err) = harness.execute_with(&inv, &harness.runner());
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("file(s) need changes. Apply them with: dx fix --apply --affected\n"),
            "{err}"
        );
        for stale in ["dx format --apply", "dx lint --apply", "//test:"] {
            assert!(!err.contains(stale), "{stale}: {err}");
        }
    }

    #[test]
    fn check_target_hints_name_dx_fix_with_the_user_patterns() {
        let harness = umbrella_findings("umbrella-target-hint");
        let (code, out, err) = harness.run(&["check", "//test:corpus", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("file(s) need changes. Apply them with: dx fix --apply //test:corpus\n"),
            "{err}"
        );
        assert!(!err.contains("dx format --apply"), "{err}");
    }

    #[test]
    fn check_bazel_failure_hint_reruns_dx_check() {
        let harness = Harness {
            bazel_code: 1,
            ..umbrella_clean("umbrella-rerun-hint")
        };
        let (code, out, err) = harness.run(&["check", "//test:corpus", "--output=text"]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(
            err.contains("next: fix the Bazel error above, then run: dx check //test:corpus\n"),
            "{err}"
        );
        assert!(!err.contains("then run: dx format"), "{err}");
    }

    #[test]
    fn check_findings_phases_keep_their_findings_in_a_complete_report() {
        let (harness, code, out, err) = check_with_failing_lint(
            "umbrella-failed-lint",
            &["check", "--output=json", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let report = *events_of_kind(&events, "report")
            .last()
            .expect("umbrella report");
        assert_eq!(report["results_complete"], serde_json::json!(true));
        let finished = *events_of_kind(&events, "command_finished")
            .last()
            .expect("umbrella finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(true));
        let runs = merged_runs(&harness);
        assert_eq!(runs.len(), 2, "lint and typecheck captures are retained");
        assert!(
            runs[0]["results"]
                .as_array()
                .is_some_and(|results| !results.is_empty()),
            "{runs:?}"
        );
        let skipped: Vec<&str> = events_of_kind(&events, "notice")
            .iter()
            .filter(|notice| notice["code"] == "umbrella_phase_skipped")
            .map(|notice| notice["correlation"].as_str().expect("correlation"))
            .collect();
        assert!(skipped.is_empty(), "{skipped:?}");
        assert!(
            events_of_kind(&events, "error").is_empty(),
            "a findings failure is not a collection failure: {out}"
        );
    }

    #[test]
    fn check_failed_phase_with_a_failed_report_write_keeps_its_exit() {
        let (_, code, out, err) = check_with_failing_lint(
            "umbrella-failed-lint-write",
            &[
                "check",
                "--output=text",
                "--report=sarif=missing-dir/out.sarif",
            ],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("report_failed"), "{err}");
    }

    #[test]
    fn collector_keeps_what_a_failed_phase_wrote() {
        let dir = temp_dir("umbrella-collector-failed");
        let good = dir.path().join("good.sarif");
        let bad = dir.path().join("bad.sarif");
        let missing = dir.path().join("missing.sarif");
        std::fs::write(
            &good,
            crate::reports::render_sarif(
                &["lint-tool".to_owned()],
                &[],
                &std::collections::BTreeMap::new(),
                true,
            )
            .expect("render"),
        )
        .expect("good capture");
        std::fs::write(&bad, b"{").expect("bad capture");
        for (path, expected) in [
            (&good, Vec::new()),
            (&bad, vec![("lint", "sarif capture is malformed")]),
            (&missing, Vec::new()),
        ] {
            let mut collector = PhaseCollector::default();
            collector.plan(Command::Lint);
            collector.require_capture(0, path.clone());
            collector.record_exit(0, 1, PhaseKind::Tool);
            collector.collect();
            assert_eq!(collector.collection_failures(), expected);
            assert!(!collector.complete());
            let document: serde_json::Value =
                serde_json::from_str(&collector.document()).expect("JSON");
            assert_eq!(
                document["runs"].as_array().expect("runs").len(),
                usize::from(path == &good)
            );
        }
    }

    #[test]
    fn collector_ignores_a_capture_a_failed_phase_never_owed() {
        let mut collector = PhaseCollector::default();
        for phase in UMBRELLA_PHASES {
            collector.plan(phase);
        }
        collector.require_capture(0, PathBuf::from("/nonexistent/format.sarif"));
        collector.record_exit(0, 1, PhaseKind::Tool);
        collector.skip_remaining(String::new());
        collector.collect();
        assert_eq!(collector.collection_failures(), Vec::new());
        assert_eq!(
            collector.reasons(),
            vec![
                "format failed".to_owned(),
                "lint skipped".to_owned(),
                "typecheck skipped".to_owned(),
                "generate skipped".to_owned(),
            ]
        );
        assert_eq!(collector.skipped(), vec!["lint", "typecheck", "generate"]);
        collector.cleanup();
    }

    #[test]
    fn check_stop_after_findings_keeps_both_facts() {
        let mut harness = umbrella_findings("umbrella-stop-after-findings");
        harness.free_disk_bytes = Some(1 << 20);
        let runner = FailFrom {
            inner: harness.runner(),
            from: 2,
            to: None,
            code: 36,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(
            &harness,
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        let (code, out, err) = harness.execute_with(&inv, &runner);
        assert_eq!(code, 36, "{out}{err}");
        assert!(
            err.contains(
                "dx: phases: format findings, lint findings, typecheck failed, generate skipped"
            ),
            "{err}"
        );
        assert!(
            out.contains("(incomplete: typecheck failed, generate skipped)"),
            "{out}"
        );
        assert_eq!(harness.seen_env.borrow().len(), 3, "generate never starts");
        assert_eq!(
            merged_runs(&harness).len(),
            1,
            "lint merges and typecheck never wrote a capture"
        );
    }

    #[test]
    fn check_later_clean_phases_never_clear_earlier_findings() {
        let harness = umbrella_findings("umbrella-later-clean");
        let (artifact, _) = harness
            .staged_artifacts()
            .into_iter()
            .next()
            .expect("artifact");
        let clean = harness.valid_result(vec![], vec![]);
        let witness = intended_witness("check", true, "", "");
        let staged = staged_runner(&harness, vec![(artifact, clean)], Some(witness), 1);
        let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
        let (code, out, err) = harness.execute_with(&inv, &staged);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let findings: Vec<&str> = events_of_kind(&events, "notice")
            .iter()
            .filter(|notice| notice["code"] == "umbrella_phase_findings")
            .map(|notice| notice["correlation"].as_str().expect("correlation"))
            .collect();
        assert_eq!(findings, vec!["check/format"]);
        let finished = *events_of_kind(&events, "command_finished")
            .last()
            .expect("umbrella finished");
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(true));
    }

    #[test]
    fn check_below_threshold_findings_keep_every_phase_clean() {
        let mut harness = umbrella_clean("umbrella-below-threshold");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![]),
        );
        let (code, out, err) = harness.run(&["check", "--output=text", "--fail-on=error"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!err.contains("dx: phases"), "{err}");
        assert_eq!(harness.seen_env.borrow().len(), 4);
    }

    #[test]
    fn check_stops_on_a_rejected_scope_before_any_later_phase() {
        let harness = umbrella_findings("umbrella-bad-scope");
        let (code, out, err) = harness.run(&["check", "--output=text", "no/such/dir"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(
            err.contains("dx: phases: format failed, lint skipped"),
            "{err}"
        );
        assert!(harness.seen_env.borrow().is_empty(), "{out}{err}");
    }

    #[test]
    fn check_reports_a_generate_exit_without_changes_as_a_tool_failure() {
        let mut harness = umbrella_clean("umbrella-generate-tool-failure");
        harness.generate_code = Some(1);
        let (code, out, err) = harness.run(&["check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("generate failed") && !err.contains("generate findings"),
            "{out}{err}"
        );
    }

    #[test]
    fn check_reports_generation_drift_after_clean_quality_phases() {
        let mut harness = umbrella_clean("umbrella-generate-drift");
        harness.generate_code = Some(1);
        harness.intended = Some(intended_witness(
            "check",
            true,
            &intended_modify("src/BUILD.bazel", b"old\n", b"new\n"),
            "",
        ));
        let (code, out, err) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let finished: Vec<(String, i64)> = events_of_kind(&events, "phase_finished")
            .iter()
            .map(|event| {
                (
                    event["phase"].as_str().expect("phase").to_owned(),
                    event["exit_code"].as_i64().expect("exit"),
                )
            })
            .collect();
        assert_eq!(
            finished,
            [
                ("format".to_owned(), 0),
                ("lint".to_owned(), 0),
                ("typecheck".to_owned(), 0),
                ("generate".to_owned(), 1)
            ]
        );
        let changes = events_of_kind(&events, "change");
        assert_eq!(changes.len(), 1, "{out}");
        assert_eq!(changes[0]["path"], "src/BUILD.bazel");
        assert_eq!(changes[0]["effect"], "proposed");
        assert!(events_of_kind(&events, "mutation").is_empty(), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n"
        );
        assert!(!harness.workspace.join("src/BUILD.bazel").exists());
        let last = events.last().expect("last");
        assert_eq!(last["exit_code"], 1, "{out}");
        assert_eq!(last["results_complete"], true, "{out}");
    }

    #[test]
    fn phase_kind_stops_only_on_selection_host_or_cancel() {
        for kind in [
            PhaseKind::Clean,
            PhaseKind::Findings,
            PhaseKind::NonConvergence,
            PhaseKind::Tool,
            PhaseKind::Incomplete,
            PhaseKind::Report,
        ] {
            assert!(kind.continues(), "{kind:?}");
            assert!(!kind.stop_reason().is_empty());
        }
        for kind in [PhaseKind::Selection, PhaseKind::Host, PhaseKind::Cancelled] {
            assert!(!kind.continues(), "{kind:?}");
            assert!(!kind.stop_reason().is_empty());
        }
    }

    #[test]
    fn parse_capture_keeps_only_well_formed_documents() {
        let document = crate::reports::render_sarif(
            &["lint-tool".to_owned()],
            &[],
            &std::collections::BTreeMap::new(),
            true,
        )
        .expect("render");
        let parsed = parse_capture(document.as_bytes()).expect("round trip");
        assert_eq!(parsed["version"], serde_json::json!("2.1.0"));
        assert_eq!(parse_capture(b"   "), Err(CaptureState::Malformed));
        assert_eq!(parse_capture(b"null"), Err(CaptureState::Malformed));
        assert_eq!(
            parse_capture(br#"{"runs":[],"$schema":"x"}"#),
            Err(CaptureState::Malformed),
            "a run needs a tool"
        );
        assert_eq!(
            parse_capture(br#"{"runs":[]}"#),
            Err(CaptureState::Malformed),
            "a document without a version is not SARIF 2.1.0"
        );
    }
}
