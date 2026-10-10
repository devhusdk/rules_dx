use super::common::*;
use super::execute;
use super::generate::execute_generate;
use crate::args::{Command, Invocation};
use crate::resolve::QueryRunner;
use dx_adopt::{load_verify_set, ResolvedStep, VerifySetError};
use dx_output::{
    command_finished, command_started, error_event, notice_event, operation_event,
    with_correlation, write_event, FinishedCounts, NoticeEvent, OutputMode,
};
use dx_process::Runner;
use std::io::Write;
use std::path::Path;

const CODE_VERIFY_SET_INVALID: &str = "verify_set_invalid";
const CODE_VERIFY_STEP_FAILED: &str = "verify_step_failed";
const CODE_VERIFY_STEP_SKIPPED: &str = "verify_step_skipped";

fn step_command(name: &str) -> Command {
    match name {
        "format" => Command::Format,
        "lint" => Command::Lint,
        "typecheck" => Command::Typecheck,
        "generate" => Command::Generate,
        "build" => Command::Build,
        "test" => Command::Test,
        _ => Command::Coverage,
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
    command: &'static str,
    required: bool,
    state: StepState,
}

#[derive(Debug, Default)]
struct StepCollector {
    steps: Vec<StepOutcome>,
}

impl StepCollector {
    fn plan(&mut self, command: &'static str, required: bool) {
        self.steps.push(StepOutcome {
            command,
            required,
            state: StepState::Planned,
        });
    }

    fn record(&mut self, index: usize, passed: bool) {
        if let Some(step) = self.steps.get_mut(index) {
            step.state = if passed {
                StepState::Passed
            } else {
                StepState::Failed
            };
        }
    }

    fn skip_remaining(&mut self) {
        for step in &mut self.steps {
            if step.state == StepState::Planned {
                step.state = StepState::Skipped;
            }
        }
    }

    fn failed_required(&self) -> bool {
        self.steps
            .iter()
            .any(|step| step.required && step.state == StepState::Failed)
    }

    fn required_skipped(&self) -> Vec<&'static str> {
        self.steps
            .iter()
            .filter(|step| step.required && step.state == StepState::Skipped)
            .map(|step| step.command)
            .collect()
    }

    fn reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        for step in &self.steps {
            match step.state {
                StepState::Failed if step.required => {
                    reasons.push(format!("{} failed", step.command));
                }
                StepState::Failed => {
                    reasons.push(format!("{} failed (optional)", step.command));
                }
                StepState::Skipped if step.required => {
                    reasons.push(format!("{} skipped", step.command));
                }
                StepState::Skipped => {
                    reasons.push(format!("{} skipped (optional)", step.command));
                }
                StepState::Planned | StepState::Passed => {}
            }
        }
        reasons
    }

    fn green(&self) -> bool {
        !self.failed_required() && self.required_skipped().is_empty()
    }

    fn summarize(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        output: OutputMode,
        chatty: bool,
        dry_run: bool,
        name: &str,
    ) {
        if self.green() {
            if output != OutputMode::Json && chatty {
                let planned = self
                    .steps
                    .iter()
                    .filter(|step| step.state == StepState::Passed)
                    .count();
                let total = self.steps.len();
                if dry_run {
                    let _ = writeln!(out, "Planned {name}: {total}/{total} steps.");
                } else {
                    let _ = writeln!(out, "Verified {name}: {planned}/{total} steps passed.");
                }
            }
            return;
        }
        let message = format!("verification {name} failed: {}", self.reasons().join(", "));
        let _ = writeln!(err, "dx: {CODE_VERIFY_STEP_FAILED}: {message}");
    }
}

fn step_correlation(step: &str) -> String {
    format!("verify/{step}")
}

fn announce_step(
    out: &mut dyn Write,
    output: OutputMode,
    step: &str,
    scopes: &[String],
) -> Result<(), i32> {
    if output != OutputMode::Json {
        return Ok(());
    }
    if let Ok(event) = operation_event("verify", step, Some(scopes)) {
        let correlated = with_correlation(event.clone(), &step_correlation(step)).unwrap_or(event);
        emit_event(out, &correlated)?;
    }
    Ok(())
}

fn announce_skip(out: &mut dyn Write, output: OutputMode, step: &str) {
    if output != OutputMode::Json {
        return;
    }
    let notice = NoticeEvent {
        level: "warning".to_owned(),
        code: CODE_VERIFY_STEP_SKIPPED.to_owned(),
        message: format!("{step} skipped after an earlier required step failed"),
        related_command: Some("verify".to_owned()),
        scope: None,
        path: None,
        language: None,
        import: None,
    };
    if let Ok(event) = notice_event(&notice) {
        let correlated = with_correlation(event.clone(), &step_correlation(step)).unwrap_or(event);
        let _ = write_event(out, &correlated);
    }
}

fn announce_optional_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    step: &str,
) {
    let message = format!("{step} failed (optional)");
    let _ = writeln!(err, "dx: {CODE_VERIFY_STEP_FAILED}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_VERIFY_STEP_FAILED, &message, None, None, Some(step)) {
            let correlated =
                with_correlation(event.clone(), &step_correlation(step)).unwrap_or(event);
            let _ = write_event(out, &correlated);
        }
    }
}

fn set_exit(error: &VerifySetError) -> i32 {
    match error {
        VerifySetError::Unreadable { .. } => 1,
        _ => 2,
    }
}

fn step_invocation(invocation: &Invocation, command: Command, step: &ResolvedStep) -> Invocation {
    let mut bazel_options = step.bazel_options.clone();
    bazel_options.extend(invocation.bazel_options.iter().cloned());
    Invocation {
        command,
        check: false,
        strict_evidence: invocation.strict_evidence,
        run_output: invocation.run_output.clone(),
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
        min_coverage: invocation.min_coverage,
        targets: step.scopes.clone(),
        bazel_options,
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

struct VerifyEnv<'a> {
    workspace: &'a Path,
    runner: &'a dyn Runner,
    query_runner: &'a dyn QueryRunner,
    temp_dir: &'a Path,
    pid: u32,
    ci: bool,
}

fn load_failure(
    out: &mut dyn Write,
    err: &mut dyn Write,
    output: OutputMode,
    error: &VerifySetError,
) -> i32 {
    let code = set_exit(error);
    let message = error.to_string();
    let _ = writeln!(err, "dx: {CODE_VERIFY_SET_INVALID}: {message}");
    if output == OutputMode::Json {
        if let Ok(event) = error_event(CODE_VERIFY_SET_INVALID, &message, None, None, None) {
            let _ = write_event(out, &event);
        }
        let _ = write_event(
            out,
            &command_finished(
                code,
                &FinishedCounts {
                    results_complete: Some(false),
                    ..FinishedCounts::default()
                },
            ),
        );
    }
    code
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
    let name = invocation.targets.first().cloned().unwrap_or_default();
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started("verify", invocation.dry_run, "check") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
    }
    let set = match load_verify_set(workspace, &name) {
        Ok(set) => set,
        Err(error) => return load_failure(out, err, invocation.output, &error),
    };
    let verify_env = VerifyEnv {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        ci,
    };
    let mut collector = StepCollector::default();
    for step in &set.steps {
        collector.plan(step_command(&step.command).name(), step.required);
    }
    let mut stdout_exit: Option<i32> = None;
    let mut stop_code: Option<i32> = None;
    for (index, step) in set.steps.iter().enumerate() {
        let command = step_command(&step.command);
        let phase_nonce = nonce.wrapping_add(index as u64);
        if let Err(exit) = announce_step(out, invocation.output, command.name(), &step.scopes) {
            stdout_exit = Some(exit);
            break;
        }
        let step_invocation = step_invocation(invocation, command, step);
        let mut step_out = Vec::new();
        let mut step_err = Vec::new();
        let code = {
            let step_env = Env {
                workspace: verify_env.workspace,
                runner: verify_env.runner,
                query_runner: verify_env.query_runner,
                temp_dir: verify_env.temp_dir,
                pid: verify_env.pid,
                nonce: phase_nonce,
                out: &mut step_out,
                err: &mut step_err,
                ci: verify_env.ci,
            };
            if command == Command::Generate {
                execute_generate(&step_invocation, step_env)
            } else {
                execute(&step_invocation, step_env)
            }
        };
        let passed = code == 0;
        collector.record(index, passed);
        let forwarded = check_stdout_write(out.write_all(&step_out))
            .and_then(|()| check_stdout_write(err.write_all(&step_err)));
        if let Err(exit) = forwarded {
            stdout_exit = Some(exit);
            break;
        }
        if !passed {
            if step.required {
                stop_code = Some(code);
                break;
            }
            announce_optional_failure(out, err, invocation.output, command.name());
        }
    }
    collector.skip_remaining();
    for step in &collector.steps {
        if step.state == StepState::Skipped {
            announce_skip(out, invocation.output, step.command);
        }
    }
    let green = collector.green();
    let code = match stdout_exit {
        Some(exit) => exit,
        None => match stop_code {
            Some(phase_code) => phase_code,
            None if green => 0,
            None => 1,
        },
    };
    if stdout_exit.is_none() {
        collector.summarize(
            out,
            err,
            invocation.output,
            invocation.chatty(),
            invocation.dry_run,
            &name,
        );
    }
    if invocation.output == OutputMode::Json {
        let _ = write_event(
            out,
            &command_finished(
                code,
                &FinishedCounts {
                    results_complete: Some(green),
                    ..FinishedCounts::default()
                },
            ),
        );
    }
    code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::test_support::strings;
    use dx_process::{ChildStatus, Runner};
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;

    const TWO_BUILDS: &str = r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build", scopes = ["//a:one"] },
  { command = "build", scopes = ["//b:two"] },
]
"#;

    struct Scripted {
        codes: RefCell<Vec<Option<i32>>>,
        seen: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Scripted {
        fn run_with(harness: &Harness, inv: &Invocation, codes: Vec<Option<i32>>) -> ScriptedRun {
            let runner = Scripted {
                codes: RefCell::new(codes),
                seen: Rc::new(RefCell::new(Vec::new())),
            };
            let (code, out, err) = harness.execute_with(inv, &runner);
            let argv = runner.seen.borrow().clone();
            ScriptedRun {
                code,
                out,
                err,
                argv,
            }
        }
    }

    struct ScriptedRun {
        code: i32,
        out: String,
        err: String,
        argv: Vec<Vec<String>>,
    }

    impl Runner for Scripted {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let launch = self.seen.borrow().len();
            self.seen.borrow_mut().push(argv.to_vec());
            let codes = self.codes.borrow();
            let code = if codes.is_empty() {
                Some(0)
            } else if launch < codes.len() {
                codes[launch]
            } else {
                codes[codes.len() - 1]
            };
            Ok(ChildStatus { code })
        }
    }

    fn harness_with(name: &str, set: &str) -> Harness {
        let harness = Harness::new(name);
        harness.write_source(dx_adopt::VERIFY_TOML_REL, set);
        harness
    }

    fn parsed(words: &[&str]) -> Invocation {
        invocation(words)
    }

    #[test]
    fn green_set_runs_every_step_in_order() {
        let harness = harness_with("verify-green", TWO_BUILDS);
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0), Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 2, "both steps launch");
        assert!(
            run.argv[0].iter().any(|arg| arg == "//a:one"),
            "first step keeps its scopes: {:?}",
            run.argv[0]
        );
        assert!(
            run.argv[1].iter().any(|arg| arg == "//b:two"),
            "second step keeps its scopes: {:?}",
            run.argv[1]
        );
        assert!(
            run.out.contains("Verified pre-pr: 2/2 steps passed."),
            "terminal summary names the set: {}",
            run.out
        );
    }

    #[test]
    fn required_failure_stops_later_steps() {
        let harness = harness_with(
            "verify-stop",
            r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build", scopes = ["//a:one"] },
  { command = "build", scopes = ["//b:two"] },
  { command = "build", scopes = ["//c:three"] },
]
"#,
        );
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0), Some(1), Some(0)]);
        assert_eq!(run.code, 1, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 2, "the third step never launches");
        assert!(
            run.err.contains("verification pre-pr failed"),
            "{}",
            run.err
        );
        assert!(run.err.contains("build failed"), "{}", run.err);
        assert!(run.err.contains("build skipped"), "{}", run.err);
    }

    #[test]
    fn optional_failure_continues_and_stays_green() {
        let harness = harness_with(
            "verify-optional",
            r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build", scopes = ["//a:one"] },
  { command = "build", scopes = ["//b:two"], required = false },
  { command = "build", scopes = ["//c:three"] },
]
"#,
        );
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0), Some(1), Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 3, "an optional failure never stops the set");
        assert!(run.err.contains("build failed (optional)"), "{}", run.err);
        assert!(
            run.out.contains("Verified pre-pr: 2/3 steps passed."),
            "the summary counts past the optional failure: {}",
            run.out
        );
    }

    #[test]
    fn step_options_run_before_invocation_options() {
        let harness = harness_with(
            "verify-merge",
            r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build", scopes = ["//a:one"], bazel_options = ["--jobs=4"] },
]
"#,
        );
        let inv = parsed(&["verify", "pre-pr", "--", "--keep_going"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 1);
        let argv = &run.argv[0];
        let step_at = argv
            .iter()
            .position(|arg| arg == "--jobs=4")
            .expect("step option launches");
        let invocation_at = argv
            .iter()
            .position(|arg| arg == "--keep_going")
            .expect("invocation option launches");
        assert!(
            step_at < invocation_at,
            "step options precede invocation options: {argv:?}"
        );
    }

    #[test]
    fn missing_step_scopes_fall_back_to_repo_wide() {
        let harness = harness_with(
            "verify-default-scope",
            r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build" },
]
"#,
        );
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert_eq!(run.argv.len(), 1);
        assert!(
            run.argv[0].iter().any(|arg| arg == "//..."),
            "empty step scopes resolve repo-wide: {:?}",
            run.argv[0]
        );
    }

    #[test]
    fn dry_run_plans_the_set_without_launching() {
        let harness = harness_with("verify-dry-run", TWO_BUILDS);
        let inv = parsed(&["verify", "pre-pr", "--dry-run"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0), Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(run.argv.is_empty(), "dry run launches nothing");
        assert!(
            run.out.contains("Planned pre-pr: 2/2 steps."),
            "a plan never claims steps passed: {}",
            run.out
        );
        assert!(
            !run.out.contains("passed"),
            "a plan never claims steps passed: {}",
            run.out
        );
    }

    #[test]
    fn json_envelope_correlates_every_step() {
        let harness = harness_with("verify-json", TWO_BUILDS);
        let inv = parsed(&["verify", "pre-pr", "--output=json"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0), Some(0)]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        let events = json_events(&run.out);
        assert_eq!(events.first().expect("first")["command"], "verify");
        assert_eq!(events.last().expect("last")["event"], "command_finished");
        assert_eq!(events.last().expect("last")["exit_code"], 0);
        assert_eq!(events.last().expect("last")["results_complete"], true);
        let operations: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == "operation")
            .collect();
        assert_eq!(operations.len(), 2, "one operation per step");
        for operation in &operations {
            assert_eq!(operation["correlation"], "verify/build");
        }
    }

    #[test]
    fn json_marks_a_failed_set_incomplete() {
        let harness = harness_with("verify-json-fail", TWO_BUILDS);
        let inv = parsed(&["verify", "pre-pr", "--output=json"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(1), Some(0)]);
        assert_eq!(run.code, 1, "{}{}", run.out, run.err);
        let events = json_events(&run.out);
        assert_eq!(events.last().expect("last")["exit_code"], 1);
        assert_eq!(events.last().expect("last")["results_complete"], false);
        let notices: Vec<&serde_json::Value> = events
            .iter()
            .filter(|event| event["event"] == "notice")
            .collect();
        assert_eq!(notices.len(), 1, "the skipped step is announced");
        assert_eq!(notices[0]["correlation"], "verify/build");
    }

    #[test]
    fn load_failures_fail_closed_without_launching() {
        let missing = Harness::new("verify-missing");
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&missing, &inv, vec![Some(0)]);
        assert_eq!(run.code, 2, "{}{}", run.out, run.err);
        assert!(run.argv.is_empty(), "no set means no launch");
        assert!(run.err.contains("dx.verify.toml"), "{}", run.err);
        let harness = harness_with("verify-unknown", TWO_BUILDS);
        let inv = parsed(&["verify", "other"]);
        let run = Scripted::run_with(&harness, &inv, vec![Some(0)]);
        assert_eq!(run.code, 2, "{}{}", run.out, run.err);
        assert!(run.argv.is_empty(), "no set means no launch");
        assert!(run.err.contains("no set \"other\""), "{}", run.err);
        assert!(
            run.err.contains("pre-pr"),
            "known sets are listed: {}",
            run.err
        );
        let broken = harness_with("verify-malformed", "schema = [broken");
        let inv = parsed(&["verify", "pre-pr"]);
        let run = Scripted::run_with(&broken, &inv, vec![Some(0)]);
        assert_eq!(run.code, 2, "{}{}", run.out, run.err);
        assert!(run.argv.is_empty(), "no set means no launch");
    }

    #[test]
    fn quality_steps_dispatch_through_the_shared_phases() {
        let harness = harness_with(
            "verify-quality",
            r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "lint", scopes = ["//..."] },
]
"#,
        );
        harness.write_source("src/a.py", "x = 1\n");
        let mut harness = harness;
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness.intended = Some(intended_witness("check", true, "", ""));
        let inv = parsed(&["verify", "pre-pr", "--output=text"]);
        let (code, out, err) = harness.execute_with(&inv, &harness.runner());
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("Verified pre-pr: 1/1 steps passed."),
            "a clean lint step verifies: {out}"
        );
    }

    #[test]
    fn parser_names_the_set_slot_and_rejects_gated_surfaces() {
        let got = crate::args::parse(&strings(&["verify", "pre-pr", "--", "--jobs=1"]))
            .expect("set plus passthrough");
        assert_eq!(got.command, Command::Verify);
        assert_eq!(got.targets, vec!["pre-pr".to_owned()]);
        assert_eq!(got.bazel_options, vec!["--jobs=1".to_owned()]);
        assert!(
            crate::args::parse(&strings(&["verify"])).is_err(),
            "the set name is required"
        );
        for words in [
            vec!["verify", "a", "b"],
            vec!["verify", "pre-pr", "--check"],
            vec!["verify", "pre-pr", "--apply"],
            vec!["verify", "pre-pr", "--report=sarif=out.sarif"],
            vec!["verify", "pre-pr", "--output=diff"],
            vec!["verify", "pre-pr", "--fail-on=error"],
            vec!["verify", "pre-pr", "--here"],
        ] {
            assert!(
                crate::args::parse(&strings(&words)).is_err(),
                "{words:?} must not parse"
            );
        }
    }
}

#[cfg(test)]
mod live_tests {
    const CONSUMER_SET: &str = r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "build", scopes = ["//app:lib"] },
]
"#;

    const CONSUMER_MODULE: &str = "module(name = \"consumer\", version = \"0.0.0\")\n";

    fn dx_binary() -> std::path::PathBuf {
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        std::path::Path::new(&root)
            .join(workspace)
            .join("cli/cli/dx")
    }

    fn consumer_root(name: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(&format!("dx-verify-live-{name}-"));
        std::fs::write(scratch.path().join("MODULE.bazel"), CONSUMER_MODULE).expect("module");
        std::fs::write(scratch.path().join("dx.verify.toml"), CONSUMER_SET).expect("set");
        scratch
    }

    fn run_dx(dir: &std::path::Path, words: &[&str]) -> (Option<i32>, String, String) {
        let output = assert_cmd::Command::new(dx_binary())
            .current_dir(dir)
            .args(words)
            .output()
            .expect("dx runs");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    #[test]
    fn live_binary_plans_a_consumer_set_without_launching() {
        let scratch = consumer_root("plan");
        let (code, out, err) = run_dx(scratch.path(), &["verify", "pre-pr", "--dry-run"]);
        assert_eq!(code, Some(0), "{out}{err}");
        let (code, out, err) = run_dx(
            scratch.path(),
            &["verify", "pre-pr", "--dry-run", "--output=json"],
        );
        assert_eq!(code, Some(0), "{out}{err}");
        assert!(out.contains("\"command\":\"verify\""), "{out}");
        assert!(out.contains("\"command_finished\""), "{out}");
    }

    #[test]
    fn live_binary_rejects_unknown_sets_as_usage_errors() {
        let scratch = consumer_root("unknown");
        let (code, _, err) = run_dx(scratch.path(), &["verify", "other", "--dry-run"]);
        assert_eq!(code, Some(2), "{err}");
        assert!(err.contains("no set \"other\""), "{err}");
        let (code, _, _) = run_dx(scratch.path(), &["verify"]);
        assert_eq!(code, Some(2), "the set name stays required");
    }
}
