use super::common::*;
use crate::args::Invocation;
use crate::plan::{plan_run, plan_run_build, BuildPlan};
use crate::reports::plan_reports;
use crate::resolve::resolve_run;
use dx_output::{command_finished, command_started, operation_event, FinishedCounts, OutputMode};
use std::io::Write;
use std::path::Path;

const WITHDRAW_SIGNAL: i32 = 15;

pub(crate) fn execute_run(invocation: &Invocation, env: Env<'_>) -> i32 {
    run_app(invocation, env, None)
}

/// Runs the app like `dx run` and stops its process tree once `withdraw` holds.
pub(crate) fn execute_run_until(
    invocation: &Invocation,
    env: Env<'_>,
    withdraw: &dyn Fn() -> bool,
) -> i32 {
    run_app(invocation, env, Some(withdraw))
}

fn run_app(invocation: &Invocation, env: Env<'_>, withdraw: Option<&dyn Fn() -> bool>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir: _,
        pid: _,
        nonce: _,
        out,
        err,
        ci: _,
        audit_today: _,
    } = env;
    let planned_reports = match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    debug_assert!(planned_reports.is_empty(), "dx run takes no --report");
    let targets = match resolve_run(
        &invocation.targets,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(targets) => targets,
        Err(error) => {
            return resolve_failed(invocation.output, out, err, &error);
        }
    };
    let launcher = Launcher {
        invocation,
        workspace,
        runner,
        withdraw,
    };
    run_targets(&launcher, out, err, &targets)
}

#[derive(Clone, Copy)]
struct Launcher<'a> {
    invocation: &'a Invocation,
    workspace: &'a Path,
    runner: &'a dyn dx_process::Runner,
    withdraw: Option<&'a dyn Fn() -> bool>,
}

impl Launcher<'_> {
    /// Builds the targets, under a slot when one is free; `Some` is the exit when dx must not launch.
    fn prebuild(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        targets: &[String],
    ) -> Option<i32> {
        let invocation = self.invocation;
        let json = invocation.output == OutputMode::Json;
        let slot = match crate::slots::take(invocation.command, &mut *err, &mut *out) {
            Ok(slot) => slot,
            Err(signo) => {
                let _ = writeln!(err, "dx: cancelled: stopped by signal {signo}");
                return Some(dx_process::interrupted_code(signo));
            }
        };
        let plan = match plan_run_build(
            targets,
            &invocation.agent_quiet_flags(),
            invocation.profile(),
            &invocation.bazel_startup_options,
        ) {
            Ok(plan) => plan,
            Err(error) => return Some(pre_exec(invocation.output, out, err, &error.to_string())),
        };
        let held = slot.as_ref().map(|slot| slot.runner(self.runner));
        let builder = Launcher {
            runner: held
                .as_ref()
                .map_or(self.runner, |held| held as &dyn dx_process::Runner),
            ..*self
        };
        let launched = builder.launch(out, err, &plan.argv);
        drop(slot);
        let code = match launched {
            Ok(Some(code)) => code,
            Ok(None) => return Some(withdrawn(invocation, out, err)),
            Err(exit) => return Some(exit),
        };
        if code == 0 {
            return None;
        }
        if json {
            if let Err(exit) = emit_built(
                out,
                bazel_exit_event(
                    CODE_BAZEL_FAILED,
                    &format!(
                        "bazel build {} exited {code} (see stderr diagnostics)",
                        targets.join(" ")
                    ),
                    code,
                    "execute",
                ),
            ) {
                return Some(kept_child(err, code, exit));
            }
            if let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default()))
            {
                return Some(kept_child(err, code, exit));
            }
        }
        Some(code)
    }

    fn plan(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        target: &str,
    ) -> Result<BuildPlan, i32> {
        let invocation = self.invocation;
        plan_run(
            target,
            &invocation.agent_quiet_flags(),
            invocation.raw_bazel_options(),
            invocation.profile(),
            &invocation.bazel_startup_options,
        )
        .map_err(|error| pre_exec(invocation.output, out, err, &error.to_string()))
    }

    fn launch_app(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        argv: &[String],
    ) -> Result<Option<AppExit>, i32> {
        match self.withdraw {
            Some(withdraw) => run_bazel_app_withdrawable(
                self.invocation,
                out,
                err,
                self.workspace,
                self.runner,
                argv,
                withdraw,
            ),
            None => run_bazel_app(
                self.invocation,
                out,
                err,
                self.workspace,
                self.runner,
                argv,
                &[],
            )
            .map(Some),
        }
    }

    fn launch(
        &self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        argv: &[String],
    ) -> Result<Option<i32>, i32> {
        match self.withdraw {
            Some(withdraw) => run_bazel_withdrawable(
                self.invocation,
                out,
                err,
                self.workspace,
                self.runner,
                argv,
                withdraw,
            ),
            None => run_bazel(
                self.invocation,
                out,
                err,
                self.workspace,
                self.runner,
                argv,
                &[],
            )
            .map(Some),
        }
    }
}

fn withdrawn(invocation: &Invocation, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let code = dx_process::interrupted_code(WITHDRAW_SIGNAL);
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default())) {
            return kept_child(err, code, exit);
        }
    }
    code
}

fn emit_run_operations(out: &mut dyn Write, command: &str, targets: &[String]) -> Result<(), i32> {
    use dx_output::with_correlation;
    for target in targets {
        let scope = [target.clone()];
        let correlation = format!("run:{target}");
        let event = operation_event(command, "execute", Some(&scope))
            .map(|event| with_correlation(event.clone(), &correlation).unwrap_or(event));
        emit_built(out, event)?;
    }
    Ok(())
}

fn application_failed_event(
    target: &str,
    code: i32,
    signal: Option<i32>,
) -> Result<serde_json::Value, dx_output::OutputError> {
    let message = match signal {
        Some(signo) => format!("bazel run {target} terminated by signal {signo}"),
        None => format!("bazel run {target} exited {code} (see stderr diagnostics)"),
    };
    let event = bazel_exit_event(CODE_APPLICATION_FAILED, &message, code, "execute")?;
    Ok(dx_output::with_correlation(event.clone(), &format!("run:{target}")).unwrap_or(event))
}

/// Writes the ending for a run: the JSON failure event and `command_finished`, or just the code in text mode.
fn finish_run(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
    failed: Option<&str>,
    exit: AppExit,
) -> i32 {
    let AppExit { code, signal } = exit;
    if invocation.output != OutputMode::Json {
        if let (Some(target), Some(signo)) = (failed, signal) {
            app_signalled_line(err, target, signo);
        }
        return code;
    }
    if let Some(target) = failed {
        if let Err(exit) = emit_built(out, application_failed_event(target, code, signal)) {
            return kept_child(err, code, exit);
        }
    }
    if let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default())) {
        return kept_child(err, code, exit);
    }
    code
}

fn run_targets(
    launcher: &Launcher<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    targets: &[String],
) -> i32 {
    let invocation = launcher.invocation;
    let json = invocation.output == OutputMode::Json;
    let mut plans = Vec::with_capacity(targets.len());
    for target in targets {
        match launcher.plan(out, err, target) {
            Ok(plan) => plans.push(plan),
            Err(exit) => return exit,
        }
    }
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
        if let Err(exit) = emit_run_operations(out, invocation.command.name(), targets) {
            return exit;
        }
    }
    if let Some(code) = launcher.prebuild(out, err, targets) {
        return code;
    }
    for (target, plan) in targets.iter().zip(&plans) {
        if !json && !invocation.quiet {
            let _ = writeln!(err, "{}", plan.summary);
        }
        let app = match launcher.launch_app(out, err, &plan.argv) {
            Ok(Some(app)) => app,
            Ok(None) => return withdrawn(invocation, out, err),
            Err(exit) => return exit,
        };
        if app.code != 0 {
            return finish_run(invocation, out, err, Some(target), app);
        }
    }
    let success = AppExit {
        code: 0,
        signal: None,
    };
    finish_run(invocation, out, err, None, success)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::args::parse;
    use crate::args::Command;
    use crate::resolve::ResolveError;

    #[test]
    fn run_label_passthrough_preserves_status_on_stderr() {
        let harness = Harness::new("run-ok");
        let (code, out, err) = harness.run(&["run", "//app:bin", "--", "--port=8080"]);
        assert_eq!(code, 0);
        assert_eq!(out, "");
        assert!(err.contains("Running run for //app:bin"), "{err}");
        let harness = Harness {
            bazel_code: 7,
            ..Harness::new("run-fails")
        };
        let (code, _, _) = harness.run(&["run", "//app:bin"]);
        assert_eq!(code, 7);
    }

    #[test]
    fn run_keeps_app_args_out_of_dx_parsing() {
        let harness = Harness::new("run-apply-args");
        let inv = invocation(&["run", "//app:bin", "--", "--apply"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 2, "{run:?}");
        assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
        assert!(!run.argv[0].contains(&"--apply".to_owned()), "{run:?}");
        assert!(run.argv[1].contains(&"run".to_owned()), "{run:?}");
        assert!(run.argv[1].contains(&"--apply".to_owned()), "{run:?}");
    }

    struct SlotRunner {
        dir: std::path::PathBuf,
        codes: Vec<i32>,
        seen: std::cell::RefCell<Vec<(Vec<String>, bool, bool)>>,
    }

    impl dx_process::Runner for SlotRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            env: &[(&str, &str)],
        ) -> std::io::Result<dx_process::ChildStatus> {
            let busy = !crate::test_support::slot_settles_free(&self.dir, 1);
            let launch = self.seen.borrow().len();
            let marked = env.contains(&(crate::slots::SLOT_ENV_VAR, "1"));
            self.seen.borrow_mut().push((argv.to_vec(), busy, marked));
            Ok(dx_process::ChildStatus::exited(
                self.codes[launch.min(self.codes.len() - 1)],
            ))
        }
    }

    fn run_under_gate(
        name: &str,
        words: &[&str],
        codes: &[i32],
    ) -> (i32, Vec<(Vec<String>, bool, bool)>, String) {
        let harness = Harness::new(name);
        let scratch = dx_test_scratch::scratch("dx-run-slot-");
        let dir = scratch.path().join("slots");
        let runner = SlotRunner {
            dir: dir.clone(),
            codes: codes.to_vec(),
            seen: std::cell::RefCell::new(Vec::new()),
        };
        let gate = crate::slots::Gate::new(dir, 1, "run-tree".to_owned(), true);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = crate::slots::with_gate(Some(gate), || {
            crate::exec::execute(
                &invocation(words),
                harness.env_with(&runner, &mut out, &mut err),
            )
        });
        (
            code,
            runner.seen.into_inner(),
            String::from_utf8(out).expect("stdout"),
        )
    }

    #[test]
    fn run_builds_under_a_slot_and_starts_the_app_without_it() {
        let (code, seen, out) = run_under_gate(
            "run-slot",
            &["run", "//app:bin", "--output=json", "--", "--port=8080"],
            &[0],
        );
        assert_eq!(code, 0, "{out}");
        assert_eq!(seen.len(), 2, "{seen:?}");
        let (build, build_busy, build_marked) = &seen[0];
        assert!(build.contains(&"build".to_owned()), "{build:?}");
        assert!(build.contains(&"//app:bin".to_owned()), "{build:?}");
        assert!(!build.contains(&"--port=8080".to_owned()), "{build:?}");
        assert!(*build_busy, "the build holds the slot");
        assert!(*build_marked, "the build sees the held slot");
        let (run, run_busy, run_marked) = &seen[1];
        assert!(run.contains(&"run".to_owned()), "{run:?}");
        assert!(run.contains(&"--port=8080".to_owned()), "{run:?}");
        assert!(!*run_busy, "the app runs without a slot");
        assert!(!*run_marked, "the app sees no held slot");
        for setting in crate::plan::BAZEL_SETTINGS {
            assert!(build.contains(&setting.to_owned()), "{build:?}");
            assert!(run.contains(&setting.to_owned()), "{run:?}");
        }
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started", "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished", "{kinds:?}");
    }

    #[test]
    fn run_build_failure_under_a_slot_skips_the_app() {
        let (code, seen, out) = run_under_gate(
            "run-slot-fail",
            &["run", "//app:bin", "--output=json"],
            &[1],
        );
        assert_eq!(code, 1, "{out}");
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].0.contains(&"build".to_owned()), "{seen:?}");
        let events = json_events(&out);
        assert_eq!(
            event(&events, "error")["code"],
            serde_json::json!("bazel_failed"),
            "{out}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1),
            "{out}"
        );
    }

    #[test]
    fn run_build_failure_without_a_slot_reports_bazel_failed() {
        for words in [
            &["run", "//app:bin", "--output=json"][..],
            &["run", "//a:bin", "//b:bin", "--output=json"][..],
        ] {
            let harness = Harness::new("run-build-fail");
            let run = harness.probe_with(&invocation(words), &[Some(1)]);
            assert_eq!(run.code, 1, "{words:?}: {run:?}");
            assert_eq!(run.argv.len(), 1, "{words:?}: {run:?}");
            assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
            let events = json_events(&run.out);
            assert_eq!(
                event(&events, "error")["code"],
                serde_json::json!("bazel_failed"),
                "{}",
                run.out
            );
            assert!(!run.out.contains("application_failed"), "{}", run.out);
            let finished = events.last().expect("finished");
            assert_eq!(finished["event"], serde_json::json!("command_finished"));
            assert_eq!(finished["exit_code"], serde_json::json!(1), "{}", run.out);
            assert!(
                harness
                    .seen_env
                    .borrow()
                    .iter()
                    .all(|env| env.iter().all(|(key, _)| key != crate::slots::SLOT_ENV_VAR)),
                "{words:?}"
            );
        }
    }

    #[test]
    fn run_rejects_apply() {
        let err = parse(&[
            "run".to_owned(),
            "--apply".to_owned(),
            "//app:bin".to_owned(),
        ])
        .expect_err("run takes no --apply");
        assert!(err.to_string().contains("--apply"), "{err}");
    }

    #[test]
    fn run_empty_scope_is_pre_exec() {
        let harness = Harness::new("run-empty");
        let (code, _, err) = harness.run(&["run"]);
        assert_eq!(code, 2);
        assert!(err.contains("empty scope"), "{err}");
    }

    #[test]
    fn run_file_without_runnable_is_operational() {
        let harness = Harness::new("run-norunnable");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness
            .query
            .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
        harness.query.script_owners("@@//pkg:lib|False\n");
        let (code, _, err) = harness.run(&["run", "pkg/a.py"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_runnable"), "{err}");
    }

    #[test]
    fn resolve_failures_map_every_run_variant_to_a_code() {
        for (error, code) in [
            (
                ResolveError::NoRunnable {
                    scopes: vec!["a".to_owned()],
                },
                "no_runnable",
            ),
            (
                ResolveError::AmbiguousRunnable {
                    candidates: vec!["//a:one".to_owned(), "//a:two".to_owned()],
                },
                "ambiguous_runnable",
            ),
            (
                ResolveError::NoTests {
                    owners: vec!["//a:lib".to_owned()],
                },
                "no_tests",
            ),
            (ResolveError::EmptyScope, "invalid_scope"),
        ] {
            assert_eq!(Failure::resolve(&error).code, code);
        }
    }

    #[test]
    fn run_ambiguous_is_operational() {
        let harness = Harness::new("run-amb");
        std::fs::create_dir_all(harness.workspace.join("app")).expect("dir");
        harness.query.script_owners("//app:two\n//app:one\n");
        harness
            .query
            .script_owners("@@//app:two|True\n@@//app:one|True\n");
        let (code, _, err) = harness.run(&["run", "app"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("ambiguous_runnable"), "{err}");
    }

    #[test]
    fn run_bad_report_is_pre_exec() {
        let args: Vec<String> = ["run", "//app:bin", "--report=sarif=a.sarif"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let err = parse(&args).expect_err("run report must fail parse");
        assert!(err.to_string().contains("--report"), "{err:?}");
    }

    #[test]
    fn run_live_json_streams_operations_and_finished() {
        let harness = Harness::new("run-live-json");
        let (code, out, err) = harness.run(&["run", "//app:bin", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let op = event(&events, "operation");
        assert_eq!(op["phase"], serde_json::json!("execute"));
        assert_eq!(op["scope"], serde_json::json!(["//app:bin"]));
        assert_eq!(op["correlation"], serde_json::json!("run://app:bin"));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn run_live_json_failure_emits_explainer() {
        let harness = Harness::new("run-live-json-fail");
        let inv = invocation(&["run", "//app:bin", "--output=json"]);
        let run = harness.probe_with(&inv, &[Some(0), Some(7)]);
        let (code, out) = (run.code, run.out);
        assert_eq!(code, 7, "{out}");
        assert!(!out.contains("bazel_failed"), "{out}");
        let events = json_events(&out);
        let error = event(&events, "error");
        assert_eq!(
            error["code"],
            serde_json::json!("application_failed"),
            "{out}"
        );
        assert_eq!(error["cause"], serde_json::json!("application"), "{out}");
        assert_eq!(error["status"], serde_json::json!(7), "{out}");
        assert_eq!(
            error["correlation"],
            serde_json::json!("run://app:bin"),
            "{out}"
        );
        assert!(out.contains("command_finished"), "{out}");
        assert!(out.contains("\"exit_code\":7"), "{out}");
    }

    #[test]
    fn run_multi_json_failure_reports_application_failed() {
        let harness = Harness::new("run-multi-json-fail");
        let inv = invocation(&["run", "//a:bin", "//b:bin", "--output=json"]);
        let run = harness.probe_with(&inv, &[Some(0), Some(7)]);
        let (code, out) = (run.code, run.out);
        assert_eq!(code, 7, "{out}");
        assert!(!out.contains("bazel_failed"), "{out}");
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["code"],
            serde_json::json!("application_failed"),
            "{out}"
        );
        assert_eq!(
            errors[0]["cause"],
            serde_json::json!("application"),
            "{out}"
        );
        assert_eq!(errors[0]["status"], serde_json::json!(7), "{out}");
        assert_eq!(
            errors[0]["correlation"],
            serde_json::json!("run://a:bin"),
            "{out}"
        );
        let finished = events.last().expect("finished");
        assert_eq!(
            finished["event"],
            serde_json::json!("command_finished"),
            "{out}"
        );
        assert_eq!(finished["exit_code"], serde_json::json!(7), "{out}");
    }

    #[test]
    fn run_app_exit_36_on_a_nearly_full_disk_is_application_failed() {
        for (name, words) in [
            ("run-app-36", &["run", "//app:bin", "--output=json"][..]),
            (
                "run-app-36-multi",
                &["run", "//a:bin", "//b:bin", "--output=json"][..],
            ),
        ] {
            let harness = Harness {
                free_disk_bytes: Some(1 << 20),
                ..Harness::new(name)
            };
            let run = harness.probe_with(&invocation(words), &[Some(0), Some(36)]);
            let (code, out) = (run.code, run.out);
            assert_eq!(code, 36, "{out}");
            assert!(!out.contains("disk_full"), "{out}");
            let events = json_events(&out);
            let errors = events_of_kind(&events, "error");
            assert_eq!(errors.len(), 1, "{out}");
            assert_eq!(
                errors[0]["code"],
                serde_json::json!("application_failed"),
                "{out}"
            );
            assert_eq!(
                errors[0]["cause"],
                serde_json::json!("application"),
                "{out}"
            );
            assert_eq!(errors[0]["status"], serde_json::json!(36), "{out}");
            let notices: Vec<_> = events_of_kind(&events, "notice")
                .into_iter()
                .filter(|notice| notice["code"] == "disk_low")
                .collect();
            assert_eq!(notices.len(), 1, "{out}");
            let finished = events.last().expect("finished");
            assert_eq!(finished["exit_code"], serde_json::json!(36), "{out}");
        }
        let harness = Harness {
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("run-app-36-text")
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let run = harness.probe_streams(
            &invocation(&["run", "//app:bin"]),
            &[Some(0), Some(36)],
            &mut out,
            &mut err,
        );
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 36, "{err}");
        assert!(!err.contains("disk_full"), "{err}");
        assert_eq!(err.matches("disk_low").count(), 1, "{err}");
    }

    #[test]
    fn run_build_exit_36_on_a_nearly_full_disk_is_disk_full() {
        let harness = Harness {
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("run-build-36")
        };
        let inv = invocation(&["run", "//app:bin", "--output=json"]);
        let run = harness.probe_with(&inv, &[Some(36)]);
        let (code, out) = (run.code, run.out);
        assert_eq!(code, 36, "{out}");
        assert_eq!(run.argv.len(), 1, "{:?}", run.argv);
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], serde_json::json!("disk_full"), "{out}");
        assert_eq!(errors[0]["cause"], serde_json::json!("host"), "{out}");
    }

    #[test]
    fn run_launch_and_signal_failures() {
        let mut harness = Harness::new("run-launch");
        harness.io_error = true;
        let (code, _, err) = harness.run(&["run", "//app:bin"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("launch_failed"), "{err}");
        let mut harness = Harness::new("run-signal");
        harness.signalled = true;
        let (code, _, err) = harness.run(&["run", "//app:bin"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_signalled"), "{err}");
    }

    #[test]
    fn run_signalled_app_exits_128_plus_the_signal_and_stops() {
        let mut harness = Harness::new("run-app-signal");
        harness.app_signal = Some(11);
        let (code, _, err) = harness.run(&["run", "//a:bin", "//b:bin"]);
        assert_eq!(code, 139, "{err}");
        assert!(
            err.contains("dx: application_failed: bazel run //a:bin terminated by signal 11"),
            "{err}"
        );
        assert!(!err.contains("bazel_signalled"), "{err}");
        assert!(!err.contains("Running run for //b:bin"), "{err}");
        assert_eq!(harness.seen_env.borrow().len(), 2, "build and one app");
    }

    #[test]
    fn run_multi_target_runs_sequential_single_plans() {
        let harness = Harness::new("run-multi");
        let (code, _, err) = harness.run(&["run", "//a:bin", "//b:bin"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running run for //a:bin"), "{err}");
        assert!(err.contains("Running run for //b:bin"), "{err}");
        let harness = Harness::new("run-multi-args");
        let (code, _, err) = harness.run(&["run", "//a:bin", "//b:bin", "--", "--port=8080"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running run for //a:bin"), "{err}");
        assert!(err.contains("Running run for //b:bin"), "{err}");
        let harness = Harness::new("run-multi-argv");
        let inv = invocation(&["run", "//a:bin", "//b:bin", "--", "--port=8080"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        assert_eq!(run.argv.len(), 3, "{run:?}");
        assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
        assert!(run.argv[0].contains(&"//a:bin".to_owned()), "{run:?}");
        assert!(run.argv[0].contains(&"//b:bin".to_owned()), "{run:?}");
        assert!(!run.argv[0].contains(&"--port=8080".to_owned()), "{run:?}");
        for argv in &run.argv[1..] {
            assert!(argv.contains(&"--port=8080".to_owned()), "{argv:?}");
        }
        assert!(run.argv[1].contains(&"//a:bin".to_owned()), "{run:?}");
        assert!(run.argv[2].contains(&"//b:bin".to_owned()), "{run:?}");
    }

    #[test]
    fn run_multi_stops_on_first_failure() {
        let harness = Harness {
            bazel_code: 7,
            ..Harness::new("run-multi-fail")
        };
        let inv = invocation(&["run", "//a:bin", "//b:bin"]);
        let run = harness.probe_with(&inv, &[Some(0), Some(7)]);
        assert_eq!(run.code, 7);
        assert_eq!(run.argv.len(), 2, "{run:?}");
    }

    #[test]
    fn run_pattern_expands_to_runnables() {
        let harness = Harness::new("run-pattern");
        harness
            .query
            .script_owners("//demo:backend\n//demo:frontend\n");
        harness
            .query
            .script_owners("@@//demo:backend|True\n@@//demo:frontend|True\n");
        let inv = invocation(&["run", "//demo/..."]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 3, "{run:?}");
        assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
        assert!(
            run.argv[1].contains(&"//demo:backend".to_owned()),
            "{run:?}"
        );
        assert!(
            run.argv[2].contains(&"//demo:frontend".to_owned()),
            "{run:?}"
        );
    }

    #[test]
    fn run_pattern_without_runnable_is_operational() {
        let harness = Harness::new("run-pattern-empty");
        harness.query.script_owners("");
        let (code, _, err) = harness.run(&["run", "//demo/..."]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_runnable"), "{err}");
    }

    fn run_invocation(
        command: Command,
        output: OutputMode,
        reports: Vec<crate::args::ReportRequest>,
    ) -> Invocation {
        let mut invocation = crate::args::parsed(&["run", "//app:bin"]);
        invocation.command = command;
        invocation.output = output;
        invocation.reports = reports;
        invocation
    }

    #[test]
    fn run_manual_invocations_cover_defense_branches() {
        let harness = Harness::new("run-manual-report");
        let inv = run_invocation(
            Command::Run,
            OutputMode::Text { quiet: false },
            vec![crate::args::ReportRequest {
                format: "junit".to_owned(),
                destination: "out.xml".to_owned(),
            }],
        );
        let (code, _, err) = harness.execute_with(&inv, &harness.runner());
        assert_eq!(code, 2, "{err}");

        let harness = Harness::new("run-manual-json");
        let inv = run_invocation(Command::Run, OutputMode::Json, Vec::new());
        let (code, out, _) = harness.execute_with(&inv, &harness.runner());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("command_started"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
    }

    #[test]
    fn run_profile_flags_reach_bazel_argv() {
        for (words, flag) in [
            (vec!["run", "//app:bin"], "--config=dx_dev"),
            (vec!["run", "--debug", "//app:bin"], "--config=dx_debug"),
            (vec!["run", "//app:bin", "--release"], "--config=dx_release"),
        ] {
            let harness = Harness::new("run-profile");
            let inv = invocation(&words);
            let run = harness.probe_with(&inv, &[Some(0)]);
            assert_eq!(run.code, 0, "{words:?}");
            assert_eq!(run.argv.len(), 2, "{words:?}");
            for argv in &run.argv {
                assert!(
                    argv.contains(&flag.to_owned()),
                    "{words:?} argv missing {flag}: {:?}",
                    run.argv
                );
            }
        }
    }

    #[test]
    fn run_ci_apply_executes_with_status() {
        let harness = Harness::new("run-ci-apply");
        let (code, _, err) = harness.run_with_ci(&["run", "//app:bin", "--", "--port=8080"], true);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running run for //app:bin"), "{err}");
        let harness = Harness {
            bazel_code: 7,
            ..Harness::new("run-ci-apply-fail")
        };
        let (code, _, _) = harness.run_with_ci(&["run", "//app:bin"], true);
        assert_eq!(code, 7);
        let harness = Harness::new("run-ci-apply-json");
        let (code, out, err) = harness.run_with_ci(&["run", "//app:bin", "--output=json"], true);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let op = event(&events, "operation");
        assert_eq!(op["phase"], serde_json::json!("execute"));
        assert_eq!(op["scope"], serde_json::json!(["//app:bin"]));
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn run_ci_keeps_resolution_errors() {
        let harness = Harness::new("run-ci-empty");
        let (code, _, err) = harness.run_with_ci(&["run"], true);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("empty scope"), "{err}");
        let harness = Harness::new("run-ci-norunnable");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness
            .query
            .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
        harness.query.script_owners("@@//pkg:lib|False\n");
        let (code, _, err) = harness.run_with_ci(&["run", "pkg/a.py"], true);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_runnable"), "{err}");
    }
    #[test]
    fn json_multirun_covers_success_and_process_failures() {
        for multiple in [false, true] {
            for scenario in ["success", "exit", "signal", "app_signal", "spawn"] {
                let mut harness = Harness::new("run-json-matrix");
                harness.bazel_code = if scenario == "exit" { 7 } else { 0 };
                harness.signalled = scenario == "signal";
                harness.app_signal = (scenario == "app_signal").then_some(11);
                harness.io_error = scenario == "spawn";
                let mut words = vec!["run", "//a:one", "--output=json"];
                if multiple {
                    words.push("//b:two");
                }
                let (code, out, err) = harness.run(&words);
                let expected = match scenario {
                    "exit" => 7,
                    "app_signal" => 139,
                    "signal" | "spawn" => 1,
                    _ => 0,
                };
                assert_eq!(code, expected, "{words:?} {scenario}: {out}{err}");
                let events: Vec<serde_json::Value> = out
                    .lines()
                    .map(|line| serde_json::from_str(line).expect("event"))
                    .collect();
                assert_eq!(events[0]["event"], "command_started");
                assert_eq!(events.last().expect("finished")["exit_code"], expected);
                if expected != 0 {
                    assert!(events.iter().any(|event| event["event"] == "error"));
                }
                if scenario == "app_signal" {
                    let errors = events_of_kind(&events, "error");
                    assert_eq!(errors.len(), 1, "{out}");
                    assert_eq!(errors[0]["code"], "application_failed", "{out}");
                    assert_eq!(errors[0]["cause"], "application", "{out}");
                    assert_eq!(errors[0]["status"], 139, "{out}");
                    assert_eq!(errors[0]["correlation"], "run://a:one", "{out}");
                    assert_eq!(
                        errors[0]["message"], "bazel run //a:one terminated by signal 11",
                        "{out}"
                    );
                    assert!(!out.contains("bazel_signalled"), "{out}");
                    assert_eq!(harness.seen_env.borrow().len(), 2, "{words:?}");
                }
            }
        }
    }

    #[test]
    fn run_quiet_suppresses_the_plan_summary() {
        for (words, expected) in [
            (vec!["run", "//app:bin"], 1),
            (vec!["run", "//app:bin", "--quiet"], 0),
            (vec!["run", "//app:bin", "//app:other", "--quiet"], 0),
        ] {
            let harness = Harness::new("run-quiet-summary");
            let (code, _out, err) = harness.run(&words);
            assert_eq!(code, 0, "{words:?}: {err}");
            assert_eq!(
                err.matches("Running run for").count(),
                expected,
                "{words:?}: {err}"
            );
        }
    }

    #[test]
    fn run_json_lifecycle_is_the_same_for_one_and_many_targets() {
        for count in [1, 2] {
            for failing in [false, true] {
                let mut harness = Harness::new("run-json-lifecycle");
                harness.bazel_code = if failing { 7 } else { 0 };
                let mut words = vec!["run", "//a:one", "--output=json"];
                if count == 2 {
                    words.push("//b:two");
                }
                let (code, out, err) = harness.run(&words);
                assert_eq!(code, if failing { 7 } else { 0 }, "{words:?}: {out}{err}");
                let events: Vec<serde_json::Value> = out
                    .lines()
                    .map(|line| serde_json::from_str(line).expect("event"))
                    .collect();
                assert_eq!(events[0]["event"], "command_started");
                let operations: Vec<&serde_json::Value> = events
                    .iter()
                    .filter(|event| event["event"] == "operation")
                    .collect();
                assert_eq!(operations.len(), count, "{out}");
                let last = events.last().expect("finished");
                assert_eq!(last["event"], "command_finished");
                assert_eq!(last["exit_code"], code);
            }
        }
    }
}
