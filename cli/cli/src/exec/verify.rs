use super::common::*;
use super::execute;
use super::generate::execute_generate;
use crate::args::{Command, Invocation};
use dx_output::{
    command_finished, command_started, error_event, notice_event, operation_event,
    with_correlation, write_event, FinishedCounts, NoticeEvent, OutputMode,
};
use std::io::Write;

const CODE_VERIFY_FAILED: &str = "verify_failed";
const CODE_VERIFY_STEP_FAILED: &str = "verify_step_failed";
const CODE_VERIFY_STEP_SKIPPED: &str = "verify_step_skipped";

fn step_command(name: &str) -> Option<Command> {
    match name {
        "format" => Some(Command::Format),
        "lint" => Some(Command::Lint),
        "typecheck" => Some(Command::Typecheck),
        "generate" => Some(Command::Generate),
        "build" => Some(Command::Build),
        "test" => Some(Command::Test),
        "coverage" => Some(Command::Coverage),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepState {
    Planned,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StepOutcome {
    command: Command,
    required: bool,
    state: StepState,
    code: Option<i32>,
}

impl StepOutcome {
    fn label(&self) -> &'static str {
        self.command.name()
    }
}

#[derive(Debug, Default)]
struct StepCollector {
    steps: Vec<StepOutcome>,
}

impl StepCollector {
    fn plan(&mut self, command: Command, required: bool) {
        self.steps.push(StepOutcome {
            command,
            required,
            state: StepState::Planned,
            code: None,
        });
    }

    fn record_exit(&mut self, index: usize, code: i32) {
        if let Some(running) = self.steps.get_mut(index) {
            running.state = if code == 0 {
                StepState::Passed
            } else {
                StepState::Failed
            };
            running.code = Some(code);
        }
    }

    fn skip_remaining(&mut self) {
        for step in &mut self.steps {
            if step.state == StepState::Planned {
                step.state = StepState::Skipped;
            }
        }
    }

    fn failed_required(&self) -> Option<&StepOutcome> {
        self.steps
            .iter()
            .find(|step| step.required && step.state == StepState::Failed)
    }

    fn skipped_required(&self) -> Vec<&'static str> {
        self.steps
            .iter()
            .filter(|step| step.required && step.state == StepState::Skipped)
            .map(StepOutcome::label)
            .collect()
    }

    fn required_passed(&self) -> usize {
        self.steps
            .iter()
            .filter(|step| step.required && step.state == StepState::Passed)
            .count()
    }

    fn complete(&self) -> bool {
        self.failed_required().is_none() && self.skipped_required().is_empty()
    }
}

fn step_correlation(index: usize, command: &str) -> String {
    format!("verify/{command}/{}", index + 1)
}

fn announce_step(
    out: &mut dyn Write,
    output: OutputMode,
    index: usize,
    command: &str,
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    if let Ok(event) = operation_event("verify", command, None) {
        let correlated =
            with_correlation(event.clone(), &step_correlation(index, command)).unwrap_or(event);
        emit_event(out, &correlated)?;
    }
    Ok(())
}

fn announce_step_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    index: usize,
    command: &str,
    code: i32,
) {
    let message = format!("step {command} failed with exit {code}");
    let _ = writeln!(err, "dx: {CODE_VERIFY_STEP_FAILED}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_VERIFY_STEP_FAILED, &message, None, None, Some(command))
        {
            let correlated =
                with_correlation(event.clone(), &step_correlation(index, command)).unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    }
}

fn announce_skip(out: &mut dyn Write, output: OutputMode, index: usize, command: &str) {
    if output != OutputMode::Json {
        return;
    }
    let notice = NoticeEvent {
        level: "warning".to_owned(),
        code: CODE_VERIFY_STEP_SKIPPED.to_owned(),
        message: format!("{command} skipped after an earlier required step failed"),
        related_command: Some("verify".to_owned()),
        scope: None,
        path: None,
        language: None,
        import: None,
    };
    if let Ok(event) = notice_event(&notice) {
        let correlated =
            with_correlation(event.clone(), &step_correlation(index, command)).unwrap_or(event);
        let _ = write_event(out, &correlated);
    }
}

fn announce_optional_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    index: usize,
    command: &str,
    code: i32,
) {
    let message = format!("optional step {command} failed with exit {code}");
    let _ = writeln!(err, "dx: warning: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_VERIFY_STEP_FAILED, &message, None, None, Some(command))
        {
            let correlated =
                with_correlation(event.clone(), &step_correlation(index, command)).unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    }
}

fn step_invocation(
    invocation: &Invocation,
    command: Command,
    scopes: &[String],
    bazel_options: &[String],
) -> Invocation {
    let mut combined = bazel_options.to_vec();
    combined.extend(invocation.bazel_options.iter().cloned());
    Invocation {
        command,
        check: true,
        strict_evidence: matches!(command, Command::Test | Command::Coverage),
        run_output: None,
        apply: false,
        debug: false,
        release: false,
        workspace: invocation.workspace.clone(),
        dry_run: invocation.dry_run,
        quiet: invocation.quiet,
        verbose: invocation.verbose,
        log_level: invocation.log_level,
        color: invocation.color,
        output: invocation.output,
        reports: Vec::new(),
        fail_on: invocation.fail_on,
        min_coverage: None,
        targets: scopes.to_vec(),
        bazel_options: combined,
        bazel_clean: false,
        prune_unobserved: false,
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
        workspace_capabilities: false,
        bazel_startup_options: invocation.bazel_startup_options.clone(),
    }
}

pub(crate) fn execute_verify(invocation: &Invocation, env: Env<'_>) -> i32 {
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
    let set_name = invocation.targets.first().cloned().unwrap_or_default();
    let set = match dx_adopt::verify::load_verify_set(workspace, &set_name) {
        Ok(set) => set,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started("verify", invocation.dry_run, "check") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
    }
    let mut collector = StepCollector::default();
    for step in &set.steps {
        let Some(command) = step_command(&step.command) else {
            return pre_exec(
                err,
                &format!(
                    "verification set {:?} names unknown command {:?}",
                    set.name, step.command
                ),
            );
        };
        collector.plan(command, step.required);
    }
    let mut stop_code: Option<i32> = None;
    for (index, step) in set.steps.iter().enumerate() {
        if stop_code.is_some() {
            break;
        }
        let command = collector.steps[index].command;
        if let Err(exit) = announce_step(out, invocation.output, index, command.name()) {
            return exit;
        }
        let step_invocation =
            step_invocation(invocation, command, &step.scopes, &step.bazel_options);
        let mut step_out = Vec::new();
        let mut step_err = Vec::new();
        let code = {
            let step_env = Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce: nonce.wrapping_add(index as u64),
                out: &mut step_out,
                err: &mut step_err,
                ci,
            };
            if command == Command::Generate {
                execute_generate(&step_invocation, step_env)
            } else {
                execute(&step_invocation, step_env)
            }
        };
        collector.record_exit(index, code);
        if let Err(exit) = check_stdout_write(out.write_all(&step_out))
            .and_then(|()| check_stdout_write(err.write_all(&step_err)))
        {
            return exit;
        }
        if code != 0 {
            if step.required {
                stop_code = Some(code);
                announce_step_failure(out, err, invocation.output, index, command.name(), code);
            } else {
                announce_optional_failure(out, err, invocation.output, index, command.name(), code);
            }
        }
    }
    collector.skip_remaining();
    let complete = collector.complete();
    for (index, step) in collector.steps.iter().enumerate() {
        if step.state == StepState::Skipped {
            announce_skip(out, invocation.output, index, step.label());
        }
    }
    if invocation.output != OutputMode::Json && invocation.chatty() {
        let passed = collector.required_passed();
        let total = collector.steps.iter().filter(|step| step.required).count();
        if complete {
            let _ = writeln!(
                out,
                "Verified set '{}': {passed} of {total} required steps passed.",
                set.name
            );
        }
    }
    let code = match stop_code {
        Some(step_code) => step_code,
        None if complete => 0,
        None => 1,
    };
    if code != 0 && stop_code.is_some() {
        let skipped = collector.skipped_required();
        let mut message = match collector.failed_required() {
            Some(failed) => format!(
                "required step {} failed with exit {}",
                failed.label(),
                failed.code.unwrap_or(1)
            ),
            None => "a required step failed".to_owned(),
        };
        if !skipped.is_empty() {
            message.push_str(&format!("; skipped required steps: {}", skipped.join(", ")));
        }
        let _ = writeln!(err, "dx: {CODE_VERIFY_FAILED}: {message}");
        if invocation.output == OutputMode::Json {
            if let Ok(event) = error_event(CODE_VERIFY_FAILED, &message, None, None, None) {
                let _ = write_event(out, &event);
            }
        }
    }
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
    use dx_process::{ChildStatus, Runner};
    use std::cell::Cell;
    use std::io;
    use std::path::Path;

    struct CodesRunner {
        inner: FakeRunner,
        codes: Vec<Option<i32>>,
        launched: Cell<usize>,
    }

    impl Runner for CodesRunner {
        fn run(
            &self,
            argv: &[String],
            cwd: &Path,
            env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let index = self.launched.get();
            self.launched.set(index + 1);
            let mut status = self.inner.run(argv, cwd, env)?;
            if index < self.codes.len() {
                status.code = self.codes[index];
            }
            Ok(status)
        }
    }

    fn run_with_codes(
        harness: &Harness,
        codes: &[Option<i32>],
        words: &[&str],
    ) -> (i32, String, String) {
        let runner = CodesRunner {
            inner: harness.runner(),
            codes: codes.to_vec(),
            launched: Cell::new(0),
        };
        let inv = invocation(words);
        harness.execute_with(&inv, &runner)
    }

    fn write_sets(harness: &Harness, text: &str) {
        std::fs::write(harness.workspace.join("dx.verify.toml"), text).expect("sets");
    }

    fn clean_quality(harness: &mut Harness) {
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness.intended = Some(intended_witness("check", true, "", ""));
    }

    fn sets_body(steps: &[&str]) -> String {
        format!(
            "schema = 1\n[sets.pre-pr]\nsteps = [\n{}\n]\n",
            steps.join(",\n")
        )
    }

    #[test]
    fn verify_runs_quality_then_workflow_steps_in_order() {
        let mut harness = Harness::new("verify-order");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&[
                "{ command = \"format\", scopes = [\"//...\"] }",
                "{ command = \"build\", scopes = [\"//...\"] }",
            ]),
        );
        let (code, out, err) = harness.run(&["verify", "pre-pr", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        let format = out
            .find("Running format analysis for //...")
            .expect("format");
        let build = out.find("Running build for //...").expect("build");
        assert!(format < build, "{out}");
        assert!(
            out.contains("Verified set 'pre-pr': 2 of 2 required steps passed."),
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            2,
            "one Bazel launch per step"
        );
    }

    #[test]
    fn verify_json_brackets_step_lifecycles() {
        let mut harness = Harness::new("verify-json");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&["{ command = \"format\", scopes = [\"//...\"] }"]),
        );
        let (code, out, err) = harness.run(&["verify", "pre-pr", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(events.first().expect("first")["event"], "command_started");
        assert_eq!(events.first().expect("first")["command"], "verify");
        assert_eq!(events.last().expect("last")["event"], "command_finished");
        assert_eq!(events.last().expect("last")["exit_code"], 0);
        assert_eq!(
            events.last().expect("last")["results_complete"],
            serde_json::json!(true)
        );
        let operations: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == "operation")
            .collect();
        assert_eq!(operations.len(), 1, "{out}");
        assert_eq!(
            operations[0]["correlation"],
            serde_json::json!("verify/format/1")
        );
    }

    #[test]
    fn verify_stops_at_first_required_failure() {
        let mut harness = Harness::new("verify-stop");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&[
                "{ command = \"format\", scopes = [\"//...\"] }",
                "{ command = \"test\", scopes = [\"//...\"] }",
                "{ command = \"build\", scopes = [\"//...\"] }",
            ]),
        );
        let (code, out, err) = run_with_codes(
            &harness,
            &[Some(0), Some(1)],
            &["verify", "pre-pr", "--output=text"],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(!out.contains("Running build for //..."), "{out}");
        assert!(
            err.contains("dx: verify_failed: required step test failed with exit 1"),
            "{err}"
        );
        assert!(err.contains("skipped required steps: build"), "{err}");
        assert!(
            !out.contains("Verified set"),
            "a failed set never claims verification: {out}"
        );
        assert_eq!(
            harness.seen_env.borrow().len(),
            2,
            "build never launches after the required test failed"
        );
    }

    #[test]
    fn verify_optional_failure_keeps_a_green_set_honest() {
        let mut harness = Harness::new("verify-optional");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&[
                "{ command = \"test\", scopes = [\"//...\"], required = false }",
                "{ command = \"format\", scopes = [\"//...\"] }",
            ]),
        );
        let (code, out, err) = run_with_codes(
            &harness,
            &[Some(1), Some(0)],
            &["verify", "pre-pr", "--output=text"],
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
        assert!(
            err.contains("dx: warning: optional step test failed with exit 1"),
            "{err}"
        );
        assert!(
            out.contains("Verified set 'pre-pr': 1 of 1 required steps passed."),
            "{out}"
        );
    }

    #[test]
    fn verify_fails_closed_on_incomplete_test_evidence() {
        let harness = Harness::new("verify-strict");
        write_sets(
            &harness,
            &sets_body(&["{ command = \"test\", scopes = [\"//...\"] }"]),
        );
        let (code, out, err) = harness.run(&["verify", "pre-pr", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("dx: verify_failed: required step test failed with exit 1"),
            "{err}"
        );
        assert!(
            !out.contains("Verified set"),
            "incomplete evidence is never green: {out}"
        );
    }

    #[test]
    fn verify_rejects_bad_config_before_any_launch() {
        for (name, sets) in [
            ("missing", None::<String>),
            ("malformed", Some("schema = [".to_owned())),
            (
                "unknown-command",
                Some(sets_body(&["{ command = \"run\", scopes = [\"//:bin\"] }"])),
            ),
            (
                "empty-scopes",
                Some(sets_body(&["{ command = \"format\", scopes = [] }"])),
            ),
        ] {
            let harness = Harness::new(&format!("verify-bad-{name}"));
            if let Some(text) = sets {
                write_sets(&harness, &text);
            }
            let (code, _, err) = harness.run(&["verify", "pre-pr", "--output=text"]);
            assert_eq!(code, 2, "{name}: {err}");
            assert!(
                harness.seen_env.borrow().is_empty(),
                "{name} launches nothing"
            );
        }
        let mut harness = Harness::new("verify-bad-set");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&["{ command = \"format\", scopes = [\"//...\"] }"]),
        );
        let (code, _, err) = harness.run(&["verify", "other", "--output=text"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("unknown verification set"), "{err}");
        assert!(harness.seen_env.borrow().is_empty(), "no launch");
    }

    #[test]
    fn verify_needs_exactly_one_set_name() {
        let harness = Harness::new("verify-arity");
        let (code, out, _) = harness.run(&["verify"]);
        assert_eq!(code, 2);
        assert_eq!(out, String::new());
        let (code, _, err) = harness.run(&["verify", "one", "two"]);
        assert_eq!(code, 2, "{err}");
        assert!(harness.seen_env.borrow().is_empty(), "no launch");
    }

    #[test]
    fn verify_dry_run_plans_without_launching() {
        let mut harness = Harness::new("verify-dry");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&[
                "{ command = \"format\", scopes = [\"//...\"] }",
                "{ command = \"build\", scopes = [\"//...\"] }",
            ]),
        );
        let (code, out, err) = harness.run(&["verify", "pre-pr", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry run launches nothing"
        );
    }

    #[test]
    fn verify_appends_cli_bazel_options_after_step_options() {
        let mut harness = Harness::new("verify-passthrough");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&[
                "{ command = \"build\", scopes = [\"//...\"], bazel_options = [\"--jobs=2\"] }",
            ]),
        );
        let inv = invocation(&["verify", "pre-pr", "--", "--jobs=4"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let step = &run.argv[0].join(" ");
        let declared = step.find("--jobs=2").expect("step option reaches bazel");
        let cli = step.find("--jobs=4").expect("cli option reaches bazel");
        assert!(declared < cli, "cli options follow step options: {step}");
    }

    #[test]
    fn verify_reads_the_committed_file_only() {
        let mut harness = Harness::new("verify-committed");
        clean_quality(&mut harness);
        write_sets(
            &harness,
            &sets_body(&["{ command = \"format\", scopes = [\"//...\"] }"]),
        );
        std::fs::write(
            harness.workspace.join("dx.local.toml"),
            "[sets.pre-pr]\nsteps = [\n{ command = \"run\", scopes = [\"//:bin\"] }\n]\n",
        )
        .expect("local file");
        let (code, out, err) = harness.run(&["verify", "pre-pr", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running format analysis for //..."), "{out}");
    }
}
