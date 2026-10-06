use super::common::*;
use super::execute;
use super::generate::execute_generate;
use crate::args::{Command, Invocation, ReportRequest};
use crate::plan::spec;
use crate::reports::plan_reports;
use dx_output::{
    command_finished, command_started, error_event, notice_event, operation_event, report_event,
    with_correlation, write_event, FinishedCounts, NoticeEvent, OutputMode,
};
use serde_json::{json, Value};
use serde_sarif::sarif::Sarif;
use std::io::Write;
use std::path::PathBuf;

const UMBRELLA_PHASES: [Command; 4] = [
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
}

impl PhaseCollector {
    /// Records one phase as planned and waiting to run.
    fn plan(&mut self, phase: Command) {
        self.phases.push(PhaseOutcome {
            phase,
            state: PhaseState::Planned,
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
    fn record_exit(&mut self, index: usize, code: i32) {
        if let Some(running) = self.phases.get_mut(index) {
            running.state = if code == 0 {
                PhaseState::Executed
            } else {
                PhaseState::Failed
            };
        }
    }

    /// Records every phase that never started as skipped.
    fn skip_remaining(&mut self) {
        for phase in &mut self.phases {
            if phase.state == PhaseState::Planned {
                phase.state = PhaseState::Skipped;
            }
        }
    }

    /// Reads and validates every capture an executed phase owed.
    fn collect(&mut self) {
        for phase in &mut self.phases {
            if phase.state != PhaseState::Executed {
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

    /// Names every phase that never started.
    fn skipped(&self) -> Vec<&'static str> {
        self.phases
            .iter()
            .filter(|phase| phase.state == PhaseState::Skipped)
            .map(|phase| phase.phase.name())
            .collect()
    }

    /// Names every executed phase whose capture could not be collected.
    fn collection_failures(&self) -> Vec<(&'static str, &'static str)> {
        self.phases
            .iter()
            .filter(|phase| phase.state == PhaseState::Executed)
            .filter_map(|phase| {
                let capture = phase.capture.as_ref()?;
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
                reasons.push(format!("{name} failed"));
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

/// Emits the correlated JSON lifecycle event for one phase the umbrella starts.
fn announce_phase(
    out: &mut dyn Write,
    output: OutputMode,
    command: &str,
    phase: &str,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    if let Ok(event) = operation_event(command, phase, None) {
        let correlated =
            with_correlation(event.clone(), &phase_correlation(command, phase)).unwrap_or(event);
        emit_event(out, &correlated)?;
    }
    Ok(())
}

/// Emits the JSON notice for one phase that never started.
fn announce_skip(out: &mut dyn Write, output: OutputMode, command: &str, phase: &str) {
    if output != OutputMode::Json {
        return;
    }
    let notice = NoticeEvent {
        level: "warning".to_owned(),
        code: "umbrella_phase_skipped".to_owned(),
        message: format!("{phase} skipped after an earlier phase failed"),
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
    let umbrella_check = invocation.command == Command::Check;
    let phase_check = umbrella_check || invocation.check;
    let mode = if phase_check { "check" } else { "default" };
    let command = invocation.command.name();
    if let Err(error) = plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        return pre_exec(err, &error.to_string());
    }
    for request in &invocation.reports {
        if request.destination == "-" {
            return pre_exec(
                err,
                &format!(
                    "option \"--report={}={}\" is not supported by dx {}: phases share one stdout document",
                    request.format,
                    request.destination,
                    command,
                ),
            );
        }
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(command, invocation.dry_run, mode) {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
    }
    let mut collector = PhaseCollector::default();
    for phase in &UMBRELLA_PHASES {
        collector.plan(*phase);
    }
    let mut stop_code: Option<i32> = None;
    let mut stdout_exit: Option<i32> = None;
    for (index, phase) in UMBRELLA_PHASES.iter().enumerate() {
        let phase_nonce = nonce.wrapping_add(index as u64);
        let mut phase_reports = Vec::new();
        let mut sarif_capture: Option<PathBuf> = None;
        for request in &invocation.reports {
            if !spec(*phase).accepts_report(&request.format) {
                continue;
            }
            if sarif_capture.is_none() {
                let capture = temp_dir.join(format!(
                    "umbrella-{}-{pid}-{phase_nonce}.sarif",
                    phase.name()
                ));
                let Some(capture_text) = capture.to_str() else {
                    return pre_exec(err, "temporary report path is not UTF-8"); // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                };
                phase_reports.push(ReportRequest {
                    format: request.format.clone(),
                    destination: capture_text.to_owned(),
                });
                sarif_capture = Some(capture);
            }
        }
        if let Err(exit) = announce_phase(out, invocation.output, command, phase.name()) {
            stdout_exit = Some(exit);
            break;
        }
        if let Some(capture) = sarif_capture {
            collector.require_capture(index, capture);
        }
        let phase_invocation = Invocation {
            command: *phase,
            check: phase_check,
            debug: false,
            release: false,
            workspace: invocation.workspace.clone(),
            dry_run: invocation.dry_run,
            quiet: invocation.quiet,
            verbose: invocation.verbose,
            log_level: invocation.log_level,
            color: invocation.color,
            output: invocation.output,
            reports: phase_reports,
            fail_on: invocation.fail_on,
            min_coverage: invocation.min_coverage,
            targets: invocation.targets.clone(),
            bazel_options: invocation.bazel_options.clone(),
            bazel_clean: false,
            pin: None,
            rollback: false,
            configured: false,
            from: None,
            to: None,
            here: false,
            serve: false,
            port: None,
            host: None,
            open: false,
            offline: false,
        };
        let mut phase_out = Vec::new();
        let mut phase_err = Vec::new();
        let code = {
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
                execute_generate(&phase_invocation, phase_env)
            } else {
                execute(&phase_invocation, phase_env)
            }
        };
        collector.record_exit(index, code);
        let forwarded = check_stdout_write(out.write_all(&phase_out))
            .and_then(|()| check_stdout_write(err.write_all(&phase_err)));
        if let Err(exit) = forwarded {
            stdout_exit = Some(exit);
            break;
        }
        if code != 0 {
            stop_code = Some(code);
            break;
        }
    }
    collector.skip_remaining();
    collector.collect();
    let complete = collector.complete();
    let reasons = collector.reasons();
    let mut reports_ok = true;
    for phase in collector.skipped() {
        announce_skip(out, invocation.output, command, phase);
    }
    for (phase, reason) in collector.collection_failures() {
        reports_ok = false;
        announce_collection_failure(out, err, invocation.output, command, phase, reason);
    }
    if stdout_exit.is_none() {
        let document = collector.document();
        for request in &invocation.reports {
            if !write_report_file(workspace, &request.destination, &document) {
                reports_ok = false;
                report_failed(
                    out,
                    err,
                    invocation.output,
                    &format!(
                        "failed to write {} report to {}",
                        request.format, request.destination
                    ),
                );
                continue;
            }
            let announced = announce_report(
                out,
                err,
                invocation.output,
                &request.format,
                &request.destination,
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
    let code = match stdout_exit {
        Some(exit) => exit,
        None => match stop_code {
            Some(phase_code) if reports_ok => phase_code,
            Some(_) => 1,
            None if reports_ok => 0,
            None => 1,
        },
    };
    if invocation.output == OutputMode::Json {
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
    use std::cell::Cell;
    use std::io::{self, Write};
    use std::path::Path;

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

    /// Rewrites one phase capture once the phase that wrote it has finished.
    struct RewriteCapture {
        inner: FakeRunner,
        capture: PathBuf,
        rewrite: Rewrite,
        done: Cell<bool>,
    }

    impl Runner for RewriteCapture {
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
        let started: Vec<&str> = events
            .iter()
            .filter(|event| event["event"] == "command_started")
            .map(|event| event["command"].as_str().expect("command"))
            .collect();
        assert_eq!(
            started,
            vec!["check", "format", "lint", "typecheck", "generate"]
        );
    }

    #[test]
    fn check_stops_at_first_failing_phase() {
        let harness = umbrella_findings("umbrella-stop");
        let (code, out, _) = harness.run(&["check", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(!out.contains("Running lint analysis"), "{out}");
        assert!(!out.contains("Running typecheck analysis"), "{out}");
        assert!(!out.contains("Running generate"), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "check mode never mutates"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "later phases never launch"
        );
    }

    #[test]
    fn check_json_stop_reports_umbrella_failure() {
        let harness = umbrella_findings("umbrella-stop-json");
        let (code, out, _) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["command"], "check");
        assert_eq!(events.last().expect("last")["event"], "command_finished");
        assert_eq!(events.last().expect("last")["exit_code"], 1);
        let started: Vec<&str> = events
            .iter()
            .filter(|event| event["event"] == "command_started")
            .map(|event| event["command"].as_str().expect("command"))
            .collect();
        assert_eq!(started, vec!["check", "format"]);
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
            1,
            "later phases never launch"
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
        let (code, out, err) = harness.run(&["fix", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
            "{out}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n"
        );
    }

    #[test]
    fn fix_stops_when_later_phase_goes_stale() {
        let harness = umbrella_findings("umbrella-stale");
        let (code, out, _) = harness.run(&["fix", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(out.contains("Running lint analysis for //..."), "{out}");
        assert!(!out.contains("Running typecheck analysis"), "{out}");
        assert!(!out.contains("Running generate"), "{out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"y = 1\n",
            "format applied before lint went stale"
        );
        let launches = harness.seen_env.borrow();
        assert_eq!(launches.len(), 2, "typecheck and generate never launch");
        assert!(
            launches
                .iter()
                .flatten()
                .all(|(key, _)| key != GENERATE_ENV_INTENDED),
            "generate dispatch never ran"
        );
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
    fn umbrella_sarif_covers_only_executed_phases() {
        let harness = umbrella_findings("umbrella-sarif-stop");
        let (code, out, err) = harness.run(&["check", "--output=json", "--report=sarif=out.sarif"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(
            events_of_kind(&events, "report")
                .last()
                .expect("umbrella report")["results_complete"],
            serde_json::json!(false),
            "a failed format phase leaves the report incomplete"
        );
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"),
        )
        .expect("SARIF JSON");
        assert_eq!(
            document["runs"].as_array().expect("runs").len(),
            0,
            "no executed SARIF-capable phase, no runs"
        );

        let (code, _, err) = harness.run(&["check", "--output=diff", "--report=sarif=out.sarif"]);
        assert_eq!(code, 1);
        assert_eq!(
            err,
            "Wrote sarif report to out.sarif (incomplete: format failed, lint skipped, \
             typecheck skipped, generate skipped).\n",
            "{err}"
        );
    }

    #[test]
    fn umbrella_sarif_skips_phase_that_could_not_render() {
        let harness = umbrella_findings("umbrella-sarif-unrendered");
        let (code, out, err) = harness.run(&["fix", "--output=text", "--report=sarif=out.sarif"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to render SARIF report"), "{err}");
        assert!(
            out.contains("Wrote sarif report to out.sarif (incomplete: lint failed"),
            "{out}"
        );
        assert!(
            out.contains("typecheck skipped, generate skipped)"),
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
        let mut harness = umbrella_clean("umbrella-fix-json");
        harness.intended = Some(intended_witness("default", true, "", ""));
        let (code, out, err) = harness.run(&["fix", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["command"], "fix");
        assert_eq!(
            events.first().expect("first")["mode"],
            "default",
            "fix mutates without --check"
        );
        assert_eq!(events.last().expect("last")["exit_code"], 0);
    }

    #[test]
    fn umbrella_diff_stops_after_first_phase_patch() {
        let harness = umbrella_findings("umbrella-diff");
        let (code, out, _) = harness.run(&["check", "--output=diff"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("src/a.py"), "{out}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "later phases never launch"
        );
    }

    #[test]
    fn check_json_correlates_one_operation_per_phase() {
        let harness = umbrella_clean("umbrella-phase-events");
        let (code, out, err) = harness.run(&["check", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let operations = events_of_kind(&events, "operation");
        let described: Vec<(&str, &str)> = operations
            .iter()
            .map(|operation| {
                (
                    operation["command"].as_str().expect("command"),
                    operation["phase"].as_str().expect("phase"),
                )
            })
            .collect();
        assert_eq!(
            described,
            vec![
                ("check", "format"),
                ("check", "lint"),
                ("check", "typecheck"),
                ("check", "generate")
            ]
        );
        for (index, phase) in ["format", "lint", "typecheck", "generate"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                operations[index]["correlation"],
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
    fn check_json_names_every_skipped_phase() {
        let harness = umbrella_findings("umbrella-skip-events");
        let (code, out, err) = harness.run(&["check", "--output=json", "--report=sarif=out.sarif"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let skipped: Vec<(&str, &str)> = events_of_kind(&events, "notice")
            .iter()
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
                    "lint skipped after an earlier phase failed"
                ),
                (
                    "umbrella_phase_skipped",
                    "typecheck skipped after an earlier phase failed"
                ),
                (
                    "umbrella_phase_skipped",
                    "generate skipped after an earlier phase failed"
                ),
            ]
        );
        for notice in events_of_kind(&events, "notice") {
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
        assert_eq!(finished["exit_code"], serde_json::json!(1));
        assert_eq!(finished["results_complete"], serde_json::json!(false));
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
        let (harness, code, out, err) = umbrella_with_broken_capture(
            harness,
            Rewrite::Remove,
            &["fix", "--output=text", "--report=sarif=out.sarif"],
        );
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
            "the first event lands, the phase event does not"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "no phase launched after stdout failed"
        );
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
        collector.record_exit(0, 0);
        collector.require_capture(1, bad.clone());
        collector.record_exit(1, 0);
        collector.require_capture(2, missing.clone());
        collector.record_exit(2, 0);
        collector.record_exit(3, 1);
        collector.skip_remaining();
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
            collector.record_exit(index, 0);
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
    fn collector_ignores_a_capture_a_failed_phase_never_owed() {
        let mut collector = PhaseCollector::default();
        for phase in UMBRELLA_PHASES {
            collector.plan(phase);
        }
        collector.require_capture(0, PathBuf::from("/nonexistent/format.sarif"));
        collector.record_exit(0, 1);
        collector.skip_remaining();
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
