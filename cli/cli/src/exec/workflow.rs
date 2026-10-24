use super::build_failures::report_build_failures;
use super::common::*;
use super::deploy::execute_deploy;
use super::failure_reports::write_failure_reports;
use super::run::execute_run;
use super::test_reports::{execute_test_reports, TestReportsRequest};
use crate::args::{Command, Invocation};
use crate::plan::{bep_path, plan_workflow, results_path, WorkflowVerb};
use crate::reports::{plan_reports, Destination, PlannedReport, StandardFormat};
use crate::resolve::{resolve, resolve_for_test_with_selection, SelectionContext};
use dx_output::{command_finished, FinishedCounts, OutputMode};
use dx_process::ForwardError;
use std::io::Write;

pub(crate) fn execute_workflow(invocation: &Invocation, env: Env<'_>) -> i32 {
    if invocation.command == Command::Run {
        return execute_run(invocation, env);
    }
    if invocation.command == Command::Deploy {
        return execute_deploy(invocation, env);
    }
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
    let code = execute_checked(
        invocation,
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
    if code == 0 {
        latched.exit_code(code)
    } else {
        code
    }
}

fn execute_checked(invocation: &Invocation, env: Env<'_>) -> i32 {
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
    let (failure_reports, planned_reports): (Vec<_>, Vec<_>) = planned_reports
        .into_iter()
        .partition(|report| report.format == StandardFormat::Failures);
    let (mut results_reports, planned_reports): (Vec<_>, Vec<_>) = planned_reports
        .into_iter()
        .partition(|report| report.format == StandardFormat::Results);
    let stdout_report = planned_reports
        .iter()
        .chain(failure_reports.iter())
        .chain(results_reports.iter())
        .any(|report| report.destination == Destination::Stdout);
    let results_default = results_reports.is_empty()
        && invocation.output == OutputMode::Json
        && invocation.command != Command::Build;
    if results_default {
        let path = results_path(workspace);
        let _ = std::fs::create_dir_all(workspace.join(".dx"));
        results_reports.push(PlannedReport {
            format: StandardFormat::Results,
            destination: Destination::File(path.to_string_lossy().into_owned()),
            path: Some(path),
        });
    }
    let resolved = match invocation.command {
        Command::Build => resolve(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ),
        Command::Test | Command::Coverage => {
            let selection = if invocation.command == Command::Test {
                SelectionContext::for_test_execution(
                    invocation.profile(),
                    &invocation.bazel_options,
                    &invocation.bazel_startup_options,
                )
            } else {
                SelectionContext::unconfigured_with_options(
                    &invocation.bazel_options,
                    &invocation.bazel_startup_options,
                )
            };
            resolve_for_test_with_selection(
                &invocation.targets,
                workspace,
                query_runner,
                &invocation.bazel_startup_options,
                &selection,
            )
        }
        _ => {
            return pre_exec(
                invocation.output,
                out,
                err,
                &ForwardError::UnsupportedCommand {
                    command: invocation.command.name().to_owned(),
                }
                .to_string(),
            );
        }
    };
    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let Some(verb) = invocation.command.workflow_verb() else {
        return pre_exec(
            invocation.output,
            out,
            err,
            &ForwardError::UnsupportedCommand {
                command: invocation.command.name().to_owned(),
            }
            .to_string(),
        );
    };
    let bep = bep_path(temp_dir, pid, nonce);
    let bep_text = bep.to_str().map(ToString::to_string);
    let Some(bep_text) = bep_text else {
        return operational(
            invocation,
            out,
            err,
            CODE_UNREADABLE_BEP,
            "temporary event path is not UTF-8",
        );
    };
    let bep_arg = Some(bep_text.as_str());
    let profile = if verb == WorkflowVerb::Coverage {
        None
    } else {
        Some(invocation.profile())
    };
    let mut bazel_options = invocation.quiet_bazel_options();
    if verb != WorkflowVerb::Build {
        match crate::exec::test_filter::rust_filter_args(
            &invocation.bazel_options,
            &resolved.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ) {
            Ok(extra) => bazel_options.extend(extra),
            Err(error) => return resolve_failed(invocation.output, out, err, &error),
        }
    }
    let plan = match plan_workflow(
        verb,
        &resolved,
        &bazel_options,
        bep_arg,
        profile,
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &format!("{error}")),
    };
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    } else if invocation.chatty() && !stdout_report {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary)) {
            return exit;
        }
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if verb == WorkflowVerb::Build {
        if bazel_code != 0 {
            report_build_failures(&bep, invocation.output, out, err);
        }
        let reports_ok = write_failure_reports(
            invocation,
            workspace,
            &bep,
            bazel_code,
            &failure_reports,
            out,
            err,
        );
        let _ = std::fs::remove_file(&bep);
        let code = if bazel_code == 0 && !reports_ok {
            1
        } else {
            bazel_code
        };
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_build_completion(invocation, out, bazel_code, code) {
                if code == 0 {
                    return exit;
                }
                let _ = writeln!(
                    err,
                    "dx: could not write the final events to stdout; keeping exit {code}"
                );
                return code;
            }
        }
        return code;
    }
    execute_test_reports(TestReportsRequest {
        invocation,
        workspace,
        out,
        err,
        verb,
        bep: &bep,
        planned_reports: &planned_reports,
        failure_reports: &failure_reports,
        results_reports: &results_reports,
        results_default,
        stdout_report,
        bazel_code,
        query_runner,
    })
}

fn emit_build_completion(
    invocation: &Invocation,
    out: &mut dyn Write,
    bazel_code: i32,
    code: i32,
) -> Result<(), i32> {
    if bazel_code != 0 {
        emit_bazel_exit(
            out,
            CODE_BAZEL_FAILED,
            &format!(
                "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                invocation.command.name(),
            ),
            bazel_code,
        )?;
    }
    emit_event(out, &command_finished(code, &FinishedCounts::default()))?;
    flush_out(out)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    #[test]
    fn build_preserves_bazel_status_verbatim() {
        let harness = Harness::new("build-ok");
        let (code, out, _) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 0);
        assert!(out.contains("Running build for //..."), "{out}");
        let harness = Harness {
            bazel_code: 3,
            ..Harness::new("build-fails")
        };
        let (code, _, _) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 3);
    }

    #[test]
    fn agent_json_runs_pass_quiet_flags_before_the_callers_options() {
        let harness = Harness::new("agent-quiet");
        let mut inv = invocation(&["test", "--output=json", "--", "--jobs=2"]);
        inv.agent = true;
        let run = harness.probe_with(&inv, &[Some(0)]);
        let argv = &run.argv[0];
        let quiet = argv
            .iter()
            .position(|arg| arg == "--noshow_progress")
            .expect("quiet flag");
        let jobs = argv.iter().position(|arg| arg == "--jobs=2").expect("jobs");
        assert!(quiet < jobs, "{argv:?}");
        for flag in crate::args::AGENT_QUIET_FLAGS {
            assert!(argv.contains(&flag.to_owned()), "{flag}: {argv:?}");
        }
        let plain = Harness::new("agent-off");
        let inv = invocation(&["test", "--output=json"]);
        let run = plain.probe_with(&inv, &[Some(0)]);
        assert!(!run.argv[0].contains(&"--noshow_progress".to_owned()));
        let mut inv = invocation(&["test", "--output=text"]);
        inv.agent = true;
        let run = plain.probe_with(&inv, &[Some(0)]);
        assert!(!run.argv[0].contains(&"--noshow_progress".to_owned()));
    }

    #[test]
    fn agent_env_test_run_is_short_quiet_and_complete() {
        let env = |name: &str| (name == "CLAUDECODE").then(|| "1".to_owned());
        let inv = crate::args::parse_with(
            &crate::test_support::strings(&["test", "//..."]),
            &env,
            &crate::args::FileDefaults::default(),
        )
        .expect("parse");
        let harness = Harness::new("agent-done");
        let run = harness.probe_with(&inv, &[Some(0)]);
        for flag in crate::args::AGENT_QUIET_FLAGS {
            assert!(
                run.argv[0].contains(&flag.to_owned()),
                "{flag}: {:?}",
                run.argv
            );
        }
        let ok_uri = write_bep_artifact(
            &harness,
            "ok.xml",
            br#"<testsuites><testsuite name="s"><testcase name="ok"/></testsuite></testsuites>"#,
        );
        let mut lines = Vec::new();
        for index in 0..200 {
            lines.push(test_result_line(
                &format!("//pkg:pass{index}"),
                &[(String::from("test.xml"), ok_uri.clone())],
            ));
        }
        let uri = write_bep_artifact(
            &harness,
            "fail.xml",
            br#"<testsuites><testsuite name="s"><testcase name="fails" classname="c"><failure message="boom">t</failure></testcase></testsuite></testsuites>"#,
        );
        lines.push(test_result_full_line(
            "//pkg:bad",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        ));
        let harness = Harness {
            bazel_code: 3,
            raw_bep: Some(lines),
            ..harness
        };
        let (_, out, err) = harness.execute_with(&inv, &harness.runner());
        assert!(out.lines().count() < 30, "{out}");
        assert!(err.is_empty(), "{err}");
        assert!(out.contains("\"failing_cases\""), "{out}");
        assert!(out.contains("boom"), "{out}");
        assert!(harness.workspace.join(".dx/results.json").is_file());
    }

    #[test]
    fn build_profile_flags_reach_bazel_argv() {
        for (words, flag) in [
            (vec!["build"], "--config=dx_dev"),
            (vec!["build", "--debug"], "--config=dx_debug"),
            (vec!["build", "--release"], "--config=dx_release"),
        ] {
            let harness = Harness::new("build-profile");
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
    fn build_forwards_consumer_configs_and_rejects_conflicting_dx_profile() {
        let harness = Harness::new("build-consumer-config");
        let inv = invocation(&["build", "--", "--config=ci", "--config=sanitizer"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let argv = &run.argv[0];
        let dev = argv
            .iter()
            .position(|arg| arg == "--config=dx_dev")
            .expect("profile reaches bazel");
        let ci = argv
            .iter()
            .position(|arg| arg == "--config=ci")
            .expect("consumer config reaches bazel");
        let sanitizer = argv
            .iter()
            .position(|arg| arg == "--config=sanitizer")
            .expect("consumer config reaches bazel");
        assert!(dev < ci && ci < sanitizer, "{argv:?}");
        let harness = Harness::new("build-conflicting-config");
        let (code, _, err) = harness.run(&["build", "--", "--config=dx_release"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("config"), "{err}");
    }

    #[test]
    fn test_and_coverage_forward_test_binary_args_to_bazel() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-test-args");
            harness.query.script_owners("py_test rule //pkg:unit\n");
            let inv = invocation(&[command, "--", "--test_arg=--exact", "--test_filter=unit"]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            assert_eq!(run.argv.len(), 1, "{command}: {run:?}");
            let argv = &run.argv[0];
            let position = argv
                .iter()
                .position(|arg| arg == "--test_arg=--exact")
                .expect("test_arg reaches bazel");
            assert_eq!(
                &argv[position..position + 2],
                &[
                    "--test_arg=--exact".to_owned(),
                    "--test_filter=unit".to_owned()
                ],
                "{command}: {argv:?}"
            );
        }
    }

    #[test]
    fn test_filter_reaches_rust_test_binaries_as_a_libtest_filter() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-rust-filter");
            harness.query.script_owners(
                "_rust_forward_test rule //cli:dx_cli_test\nrust_test rule //cli:raw_test\n",
            );
            let inv = invocation(&[command, "--", "--test_filter=slots"]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            let queries = harness.query.calls.borrow();
            assert_eq!(queries.len(), 1, "{command}: {queries:?}");
            let query = &queries[0];
            assert!(query.contains(&"query".to_owned()), "{query:?}");
            assert!(query.contains(&"--output=label_kind".to_owned()), "{query:?}");
            assert_eq!(
                query.last().map(String::as_str),
                Some("tests(\"//...\")"),
                "{query:?}"
            );
            let argv = &run.argv[0];
            let filter = argv
                .iter()
                .position(|arg| arg == "--test_filter=slots")
                .expect("test_filter reaches bazel");
            let test_arg = argv
                .iter()
                .position(|arg| arg == "--test_arg=slots")
                .expect("filter reaches the rust test binary");
            assert!(filter < test_arg, "{command}: {argv:?}");
        }
    }

    #[test]
    fn test_filter_stays_a_bazel_option_for_mixed_or_empty_selections() {
        for kinds in ["_rust_forward_test rule //cli:unit\npy_test rule //py:unit\n", ""] {
            let harness = Harness::new("wf-mixed-filter");
            harness.query.script_owners(kinds);
            let inv = invocation(&["test", "--", "--test_filter", "unit"]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            let argv = &run.argv[0];
            assert!(
                !argv.iter().any(|arg| arg.starts_with("--test_arg")),
                "{kinds:?}: {argv:?}"
            );
            assert!(argv.contains(&"--test_filter".to_owned()), "{argv:?}");
        }
        let harness = Harness::new("wf-no-filter");
        let inv = invocation(&["test", "--", "--jobs=2"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert!(harness.query.calls.borrow().is_empty());
        assert!(!run.argv[0].iter().any(|arg| arg.starts_with("--test_arg")));
    }

    #[test]
    fn build_still_rejects_test_binary_args() {
        let harness = Harness::new("wf-build-test-args");
        let (code, _, err) = harness.run(&["build", "--", "--test_arg=--exact"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("test_arg"), "{err}");
    }

    #[test]
    fn workflow_bad_report_is_pre_exec() {
        let harness = Harness::new("wf-bad-report");
        let (code, _, err) = harness.run(&["build", "--report=junit=a.xml"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn workflow_bad_scope_is_pre_exec() {
        let harness = Harness::new("wf-bad-scope");
        let (code, _, err) = harness.run(&["build", "nope.py"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn workflow_conflicting_option_is_pre_exec() {
        let harness = Harness::new("wf-conflict");
        let (code, _, err) = harness.run(&["build", "--", "--home_rc"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    #[cfg(unix)]
    fn workflow_non_utf8_bep_is_operational() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;
        let mut harness = Harness::new("wf-nonutf8");
        harness.temp = PathBuf::from(OsString::from_vec(vec![0xff]));
        let (code, _, err) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("unreadable_bep"), "{err}");
    }

    #[test]
    fn workflow_json_lifecycle_and_build_status() {
        let harness = Harness::new("wf-json");
        let (code, out, _) = harness.run(&["build", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("command_started"), "{out}");
        assert!(out.contains("command_finished"), "{out}");
    }

    #[test]
    fn workflow_launch_failure_is_operational() {
        let mut harness = Harness::new("wf-launch");
        harness.io_error = true;
        let (code, _, err) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("launch_failed"), "{err}");
    }

    #[test]
    fn workflow_signalled_is_operational() {
        let mut harness = Harness::new("wf-signal");
        harness.signalled = true;
        let (code, _, err) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bazel_signalled"), "{err}");
    }

    #[test]
    fn startup_options_reach_query_and_build_argv() {
        let harness = Harness::new("wf-startup");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness.query.script_owners("//pkg:lib\n");
        let inv = invocation(&[
            "build",
            "--bazel-startup-option=--output_base=/tmp/a",
            "--bazel-startup-option=--output_user_root=/tmp/b",
            "pkg/a.py",
        ]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 1, "one ownership query");
        assert_eq!(
            queries[0],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "--output_user_root=/tmp/b".to_owned(),
                "query".to_owned(),
                "--".to_owned(),
                "kind('rule', rdeps(//..., set(\"//pkg:a.py\"), 1))".to_owned(),
            ]
        );
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let build = &run.argv[0];
        let verb = build
            .iter()
            .position(|arg| arg == "build")
            .expect("build verb");
        assert_eq!(
            &build[..verb],
            &[
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "--output_user_root=/tmp/b".to_owned(),
            ]
        );
    }

    #[test]
    fn invalid_startup_options_fail_before_launch() {
        let harness = Harness::new("wf-startup-bad");
        let (code, _, err) =
            harness.run(&["build", "--bazel-startup-option=--jobs=4", "//app:bin"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("\"jobs\""), "{err}");
        assert!(harness.query.calls.borrow().is_empty());
    }

    #[test]
    fn test_file_scope_selection_shares_execution_configuration() {
        let harness = Harness::new("wf-test-selection");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness.query.script_owners("//pkg:lib\n");
        harness.query.script_owners("//pkg:lib (abc)\n");
        harness.query.script_owners("//pkg:unit\n");
        harness.query.script_owners("//pkg:unit (abc)\n");
        let inv = invocation(&["test", "pkg/a.py", "--", "--platforms=//:x"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 4, "ownership plus mapping: {queries:?}");
        assert_eq!(
            queries[0][3], "query",
            "ownership stays unconfigured: {queries:?}"
        );
        assert_eq!(
            queries[1][3], "cquery",
            "test mapping uses execution configuration: {queries:?}"
        );
        assert!(
            queries[1].contains(&"--platforms=//:x".to_owned()),
            "mapping shares platforms with execution: {queries:?}"
        );
        assert!(
            queries[1].contains(&"--config=dx_dev".to_owned()),
            "mapping shares profile with execution: {queries:?}"
        );
        assert!(
            !queries[0].contains(&"--platforms=//:x".to_owned()),
            "ownership ignores configuration: {:?}",
            queries[0]
        );
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let argv = &run.argv[0];
        assert!(argv.contains(&"--platforms=//:x".to_owned()), "{argv:?}");
        assert!(argv.contains(&"--config=dx_dev".to_owned()), "{argv:?}");
    }

    #[test]
    fn test_file_scope_conflicting_profile_is_pre_exec() {
        let harness = Harness::new("wf-test-conflict");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness.query.script_owners("//pkg:lib\n");
        let (code, _, err) = harness.run(&["test", "pkg/a.py", "--", "--config=dx_release"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("config"), "{err}");
    }
}

#[cfg(test)]
#[cfg(unix)]
#[path = "stream_probe_tests.rs"]
mod stream_probe_tests;

#[cfg(test)]
#[cfg(unix)]
#[path = "build_failure_launcher_tests.rs"]
mod build_failure_launcher_tests;

#[cfg(test)]
#[cfg(unix)]
#[path = "test_case_launcher_tests.rs"]
mod test_case_launcher_tests;

#[cfg(test)]
#[cfg(unix)]
#[path = "error_boundary_launcher_tests.rs"]
mod error_boundary_launcher_tests;

#[cfg(test)]
#[path = "workflow_delivery_tests.rs"]
mod workflow_delivery_tests;
