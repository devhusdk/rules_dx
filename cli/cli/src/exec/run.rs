use super::common::*;
use crate::args::Invocation;
use crate::plan::plan_run;
use crate::reports::plan_reports;
use crate::resolve::{resolve_run, ResolveError};
use dx_output::{
    command_finished, command_started, error_event, operation_event, write_event, FinishedCounts,
    OutputMode,
};
use std::io::Write;
use std::path::Path;

const CODE_NO_RUNNABLE: &str = "no_runnable";
const CODE_AMBIGUOUS_RUNNABLE: &str = "ambiguous_runnable";
const CODE_NO_TESTS: &str = "no_tests";

fn resolve_code(error: &ResolveError) -> &'static str {
    match error {
        ResolveError::NoRunnable { .. } => CODE_NO_RUNNABLE,
        ResolveError::AmbiguousRunnable { .. } => CODE_AMBIGUOUS_RUNNABLE,
        ResolveError::NoTests { .. } => CODE_NO_TESTS,
        _ => "scope_error",
    }
}

pub(crate) fn execute_run(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir: _,
        pid: _,
        nonce: _,
        out,
        err,
        ci,
    } = env;
    if ci {
        return pre_exec(err, "dx run refuses when CI=true: local-only command");
    }
    let planned_reports = match plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    debug_assert!(planned_reports.is_empty(), "dx run takes no --report");
    let targets = match resolve_run(&invocation.targets, workspace, query_runner) {
        Ok(targets) => targets,
        Err(error) => {
            let code = resolve_code(&error);
            let message = error.to_string();
            if matches!(
                error,
                ResolveError::NoRunnable { .. } | ResolveError::AmbiguousRunnable { .. }
            ) {
                return operational(invocation, out, err, code, &message);
            }
            return pre_exec(err, &message);
        }
    };
    if targets.len() == 1 {
        return execute_run_single(invocation, workspace, runner, out, err, &targets[0]);
    }
    execute_run_multi(invocation, workspace, runner, out, err, &targets)
}

fn emit_run_operations(out: &mut dyn Write, command: &str, targets: &[String]) {
    use dx_output::with_correlation;
    for target in targets {
        let scope = [target.clone()];
        if let Ok(event) = operation_event(command, "execute", Some(&scope)) {
            let correlation = format!("run:{target}");
            let event = with_correlation(event.clone(), &correlation).unwrap_or(event);
            let _ = write_event(out, &event);
        }
    }
}

fn execute_run_single(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn dx_process::Runner,
    out: &mut dyn Write,
    err: &mut dyn Write,
    target: &str,
) -> i32 {
    let plan = plan_run(target, &invocation.bazel_options, invocation.profile());
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            emit_run_operations(out, invocation.command.name(), &[target.to_owned()]);
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if !invocation.quiet {
            let _ = writeln!(err, "{}", plan.summary);
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
        emit_run_operations(out, invocation.command.name(), &[target.to_owned()]);
        let code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
            Ok(code) => code,
            Err(exit) => return exit,
        };
        if code != 0 {
            if let Ok(event) = error_event(
                "bazel_failed",
                &format!("application {target} failed with exit {code} (see stderr diagnostics)"),
                None,
                None,
                Some("execute"),
            ) {
                let _ = write_event(out, &event);
            }
        }
        let _ = write_event(out, &command_finished(code, &FinishedCounts::default()));
        return code;
    }
    if !invocation.quiet {
        let _ = writeln!(err, "{}", plan.summary);
    }
    match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => exit,
    }
}

fn execute_run_multi(
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn dx_process::Runner,
    out: &mut dyn Write,
    err: &mut dyn Write,
    targets: &[String],
) -> i32 {
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            emit_run_operations(out, invocation.command.name(), targets);
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if !invocation.quiet {
            for target in targets {
                let plan = plan_run(target, &invocation.bazel_options, invocation.profile());
                let _ = writeln!(err, "{}", plan.summary);
            }
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
        emit_run_operations(out, invocation.command.name(), targets);
        for target in targets {
            let plan = plan_run(target, &invocation.bazel_options, invocation.profile());
            let code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
                Ok(code) => code,
                Err(exit) => return exit,
            };
            if code != 0 {
                if let Ok(event) = error_event(
                    "bazel_failed",
                    &format!(
                        "application {target} failed with exit {code} (see stderr diagnostics)"
                    ),
                    None,
                    None,
                    Some("execute"),
                ) {
                    let _ = write_event(out, &event);
                }
                let _ = write_event(out, &command_finished(code, &FinishedCounts::default()));
                return code;
            }
        }
        let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        return 0;
    }
    for target in targets {
        let plan = plan_run(target, &invocation.bazel_options, invocation.profile());
        if !invocation.quiet {
            let _ = writeln!(err, "{}", plan.summary);
        }
        let code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
            Ok(code) => code,
            Err(exit) => return exit,
        };
        if code != 0 {
            return code;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::args::parse;
    use crate::args::Command;

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
        harness.query.script_owners("");
        harness.query.script_owners("//pkg:lib\n");
        let (code, _, err) = harness.run(&["run", "pkg/a.py"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no_runnable"), "{err}");
    }

    #[test]
    fn resolve_code_maps_all_variants() {
        assert_eq!(
            resolve_code(&ResolveError::NoRunnable {
                scopes: vec!["a".to_owned()],
            }),
            "no_runnable"
        );
        assert_eq!(
            resolve_code(&ResolveError::AmbiguousRunnable {
                candidates: vec!["//a:one".to_owned(), "//a:two".to_owned()],
            }),
            "ambiguous_runnable"
        );
        assert_eq!(
            resolve_code(&ResolveError::NoTests {
                owners: vec!["//a:lib".to_owned()],
            }),
            "no_tests"
        );
        assert_eq!(resolve_code(&ResolveError::EmptyScope), "scope_error");
    }

    #[test]
    fn run_ambiguous_is_operational() {
        let harness = Harness::new("run-amb");
        std::fs::create_dir_all(harness.workspace.join("app")).expect("dir");
        harness.query.script_owners("//app:two\n//app:one\n");
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
    fn run_dry_run_json_and_text() {
        let harness = Harness::new("run-dry-json");
        let (code, out, err) = harness.run(&["run", "//app:bin", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("command_started"), "{out}");
        assert!(out.contains("\"phase\":\"execute\""), "{out}");
        assert!(out.contains("//app:bin"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert_eq!(err, "", "{err}");
        let harness = Harness::new("run-dry-text");
        let (code, _, err) = harness.run(&["run", "//app:bin", "--dry-run"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running run"), "{err}");
    }

    #[test]
    fn run_live_json_streams_operations_and_finished() {
        let harness = Harness::new("run-live-json");
        let (code, out, err) = harness.run(&["run", "//app:bin", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let op = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("operation"))
            .expect("operation");
        assert_eq!(op["phase"], serde_json::json!("execute"));
        assert_eq!(op["scope"], serde_json::json!(["//app:bin"]));
        assert_eq!(op["correlation"], serde_json::json!("run://app:bin"));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        for line in out.lines() {
            serde_json::from_str::<serde_json::Value>(line).expect("NDJSON line");
        }
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn run_live_json_failure_emits_explainer() {
        let harness = Harness {
            bazel_code: 7,
            ..Harness::new("run-live-json-fail")
        };
        let (code, out, _) = harness.run(&["run", "//app:bin", "--output=json"]);
        assert_eq!(code, 7, "{out}");
        assert!(out.contains("bazel_failed"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
        assert!(out.contains("\"exit_code\":7"), "{out}");
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
        assert_eq!(run.argv.len(), 2, "{run:?}");
        for argv in &run.argv {
            assert!(argv.contains(&"--port=8080".to_owned()), "{argv:?}");
        }
        assert!(run.argv[0].contains(&"//a:bin".to_owned()), "{run:?}");
        assert!(run.argv[1].contains(&"//b:bin".to_owned()), "{run:?}");
    }

    #[test]
    fn run_multi_stops_on_first_failure() {
        let harness = Harness {
            bazel_code: 7,
            ..Harness::new("run-multi-fail")
        };
        let inv = invocation(&["run", "//a:bin", "//b:bin"]);
        let run = harness.probe_with(&inv, &[Some(7)]);
        assert_eq!(run.code, 7);
        assert_eq!(run.argv.len(), 1, "{run:?}");
    }

    #[test]
    fn run_pattern_expands_to_runnables() {
        let harness = Harness::new("run-pattern");
        harness
            .query
            .script_owners("//demo:backend\n//demo:frontend\n");
        let inv = invocation(&["run", "//demo/..."]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 2, "{run:?}");
        assert!(
            run.argv[0].contains(&"//demo:backend".to_owned()),
            "{run:?}"
        );
        assert!(
            run.argv[1].contains(&"//demo:frontend".to_owned()),
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

    #[test]
    fn run_multi_dry_run_lists_each_target() {
        let harness = Harness::new("run-multi-dry");
        let (code, _, err) = harness.run(&["run", "//a:bin", "//b:bin", "--dry-run"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running run for //a:bin"), "{err}");
        assert!(err.contains("Running run for //b:bin"), "{err}");
    }

    fn run_invocation(
        command: Command,
        output: OutputMode,
        reports: Vec<crate::args::ReportRequest>,
        dry_run: bool,
    ) -> Invocation {
        Invocation {
            command,
            check: false,
            debug: false,
            release: false,
            workspace: None,
            dry_run,
            quiet: false,
            verbose: false,
            log_level: None,
            color: dx_output::ColorMode::Auto,
            output,
            reports,
            fail_on: dx_output::Threshold::Warning,
            min_coverage: None,
            targets: vec!["//app:bin".to_owned()],
            bazel_options: Vec::new(),
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
        }
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
            false,
        );
        let (code, _, err) = harness.execute_with(&inv, &harness.runner());
        assert_eq!(code, 2, "{err}");

        let harness = Harness::new("run-manual-json");
        let inv = run_invocation(Command::Run, OutputMode::Json, Vec::new(), true);
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
            assert_eq!(run.argv.len(), 1, "{words:?}");
            assert!(
                run.argv[0].contains(&flag.to_owned()),
                "{words:?} argv missing {flag}: {:?}",
                run.argv
            );
        }
    }

    #[test]
    fn run_ci_refusal_is_pre_exec() {
        let harness = Harness::new("run-ci");
        let (code, _, err) = harness.run_with_ci(&["run", "//app:bin"], true);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("CI=true"), "{err}");
        std::fs::create_dir_all(harness.workspace.join("app")).expect("dir");
        harness.query.script_owners("//app:two\n//app:one\n");
        let (code, _, err) = harness.run(&["run", "app"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("ambiguous_runnable"), "{err}");
    }
    #[test]
    fn json_multirun_covers_success_dry_run_and_process_failures() {
        for multiple in [false, true] {
            for scenario in ["success", "dry", "exit", "signal", "spawn"] {
                let mut harness = Harness::new("run-json-matrix");
                harness.bazel_code = if scenario == "exit" { 7 } else { 0 };
                harness.signalled = scenario == "signal";
                harness.io_error = scenario == "spawn";
                let mut words = vec!["run", "//a:one", "--output=json"];
                if multiple {
                    words.push("//b:two");
                }
                if scenario == "dry" {
                    words.push("--dry-run");
                }
                let (code, out, err) = harness.run(&words);
                let expected = match scenario {
                    "exit" => 7,
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
                if scenario == "dry" {
                    assert!(harness.seen_env.borrow().is_empty());
                } else if expected != 0 {
                    assert!(events.iter().any(|event| event["event"] == "error"));
                }
            }
        }
    }
}
