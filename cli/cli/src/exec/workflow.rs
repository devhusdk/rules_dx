use super::build_failures::report_build_failures;
use super::common::*;
use super::deploy::execute_deploy;
use super::failure_reports::write_failure_reports;
use super::profile_report;
use super::run::execute_run;
use super::skipped_targets::{group_skipped, read_skipped, skipped_notice};
use super::test_reports::{TestReportsRequest, execute_test_reports};
use dx_output::{
    ActionCounts, FinishedCounts, NoticeEvent, OutputMode, command_finished, notice_event,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::plan::{WorkflowVerb, bep_path, plan_workflow, results_path, run_id};
use dx_plan::reports::{Destination, PlannedReport, StandardFormat, plan_reports};
use dx_plan::resolve::{SelectionContext, resolve, resolve_for_test_with_selection};
use dx_process::ForwardError;
use std::io::Write;

pub(crate) fn execute_workflow(invocation: &Invocation, env: Env<'_>) -> i32 {
    run_workflow(invocation, env, &mut false, &mut false)
}

/// Runs one workflow command and reports its exit code, whether its test results are complete,
/// and whether Bazel failed on the host (launch failure, signal, or a full disk).
pub(crate) fn execute_workflow_phase(invocation: &Invocation, env: Env<'_>) -> (i32, bool, bool) {
    let mut complete = false;
    let mut host = false;
    let code = run_workflow(invocation, env, &mut complete, &mut host);
    (code, complete, host)
}

fn run_workflow(
    invocation: &Invocation,
    env: Env<'_>,
    complete: &mut bool,
    host: &mut bool,
) -> i32 {
    if invocation.command == Command::Run {
        return execute_run(invocation, env);
    }
    if invocation.command == Command::Deploy {
        return execute_deploy(invocation, env);
    }
    execute_checked(invocation, env, complete, host)
}

fn execute_checked(
    invocation: &Invocation,
    env: Env<'_>,
    complete: &mut bool,
    host: &mut bool,
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
        audit_today: _,
        actions,
        proc_root: _,
        edit_commit_json: _,
    } = env;
    let results_default = invocation.output == OutputMode::Json
        && invocation.command != Command::Build
        && !invocation
            .reports
            .iter()
            .any(|report| StandardFormat::parse(&report.format) == Some(StandardFormat::Results));
    let run_id = run_id(pid, nonce, temp_dir);
    if results_default {
        let _ = std::fs::remove_file(results_path(workspace));
    }
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
    if results_default {
        let path = results_path(workspace);
        let _ = std::fs::create_dir_all(dx_path::state::local_dir(workspace));
        results_reports.push(PlannedReport {
            format: StandardFormat::Results,
            destination: Destination::File(path.to_string_lossy().into_owned()),
            path: Some(path),
        });
    }
    let selection = match invocation.command {
        Command::Test => Some(SelectionContext::for_test_execution(
            invocation.build_mode(),
            invocation.raw_bazel_options(),
            &invocation.bazel_startup_options,
        )),
        Command::Coverage => Some(SelectionContext::unconfigured_with_options(
            invocation.raw_bazel_options(),
            &invocation.bazel_startup_options,
        )),
        _ => None,
    };
    let resolved = match (invocation.command, &selection) {
        (Command::Build, _) => resolve(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ),
        (Command::Test | Command::Coverage, Some(selection)) => resolve_for_test_with_selection(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
            selection,
        ),
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
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let bep_arg = Some(bep_text.as_str());
    let build_mode = if verb == WorkflowVerb::Coverage {
        None
    } else {
        Some(invocation.build_mode())
    };
    let mut bazel_options = invocation.quiet_bazel_options();
    if let Some(selection) = selection.as_ref().filter(|_| verb != WorkflowVerb::Build) {
        match crate::exec::test_filter::rust_filter_args(
            invocation.raw_bazel_options(),
            &resolved.targets,
            workspace,
            query_runner,
            selection,
        ) {
            Ok(extra) => bazel_options.extend(extra),
            Err(error) => {
                return resolve_failed(invocation.output, out, err, &error);
            }
        }
    }
    let trace = if invocation.timings {
        match prepare_trace(workspace, &run_id) {
            Ok(path) => {
                bazel_options.push(format!("--profile={path}"));
                bazel_options
                    .push("--experimental_collect_local_sandbox_action_metrics".to_string());
                Some(path)
            }
            Err(message) => {
                return operational(
                    invocation,
                    out,
                    err,
                    profile_report::CODE_PROFILE_UNREADABLE,
                    &message,
                );
            }
        }
    } else {
        None
    };
    let plan = match plan_workflow(
        verb,
        &resolved,
        &bazel_options,
        bep_arg,
        build_mode,
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return pre_exec(invocation.output, out, err, &format!("{error}"));
        }
    };
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    } else if invocation.chatty()
        && !stdout_report
        && let Err(exit) = check_stdout_write(writeln!(out, "{}", plan.summary))
    {
        return exit;
    }
    if let Err(exit) = warn_scope_excluded(invocation.output, out, err, &resolved.kept) {
        return exit;
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => {
            let _ = std::fs::remove_file(&bep);
            *host = true;
            return exit;
        }
    };
    let action_counts = read_actions(&bep, actions);
    if let Some(trace) = &trace
        && let Err(exit) = announce_profile(
            invocation,
            workspace,
            out,
            err,
            trace,
            Some(&bep),
            action_counts,
        )
    {
        let _ = std::fs::remove_file(&bep);
        return kept_child(err, bazel_code, exit);
    }
    if verb == WorkflowVerb::Build {
        if bazel_code != 0
            && let Err(exit) = report_build_failures(&bep, invocation.output, out, err)
        {
            let _ = std::fs::remove_file(&bep);
            return kept_child(err, bazel_code, exit);
        }
        let groups = group_skipped(&read_skipped(&bep), invocation, workspace, query_runner);
        if let Err(exit) = skipped_notice(invocation.output, out, err, &groups) {
            let _ = std::fs::remove_file(&bep);
            return kept_child(err, bazel_code, exit);
        }
        let reports = write_failure_reports(
            invocation,
            workspace,
            &bep,
            bazel_code,
            &failure_reports,
            out,
            err,
        );
        let _ = std::fs::remove_file(&bep);
        let reports_ok = match reports {
            Ok(landed) => landed,
            Err(exit) => return kept_child(err, bazel_code, exit),
        };
        let code = if bazel_code == 0 && !reports_ok {
            1
        } else {
            bazel_code
        };
        if invocation.output == OutputMode::Json
            && let Err(exit) =
                emit_build_completion(invocation, out, bazel_code, code, action_counts)
        {
            if code == 0 {
                return exit;
            }
            let _ = writeln!(
                err,
                "dx: could not write the final events to stdout; keeping exit {code}"
            );
            return code;
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
        run_id: &run_id,
        stdout_report,
        bazel_code,
        query_runner,
        results_complete: complete,
        actions: action_counts,
        whole_repo: resolved.targets == ["//..."],
    })
}

/// Creates `.dx/local/timings` and returns the trace file Bazel should write.
pub(super) fn prepare_trace(workspace: &std::path::Path, run_id: &str) -> Result<String, String> {
    let dir = profile_report::dir(workspace);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    profile_report::prune(workspace);
    profile_report::profile_path(workspace, run_id)
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "profile path is not UTF-8".to_owned())
}

/// Reads the trace Bazel wrote and reports where the time went: text on stderr, or a notice
/// pointing at `.dx/local/timings/report.json` in JSON.
pub(super) fn announce_profile(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
    trace: &str,
    bep: Option<&std::path::Path>,
    counts: Option<ActionCounts>,
) -> Result<(), i32> {
    let cache = counts.map(|counts| profile_report::CacheUse {
        executed: counts.executed,
        disk: counts.disk,
        remote: counts.remote,
        local: counts.local,
    });
    let bytes = bep
        .and_then(|bep| std::fs::File::open(bep).ok())
        .and_then(|file| dx_bep::collect_bytes_received(std::io::BufReader::new(file)));
    let totals = bep
        .and_then(|bep| std::fs::File::open(bep).ok())
        .and_then(|file| dx_bep::collect_build_totals(std::io::BufReader::new(file)));
    let report = profile_report::read_trace(std::path::Path::new(trace))
        .map_err(|error| error.to_string())
        .and_then(|text| {
            profile_report::build(&text, cache, bytes).map_err(|error| error.to_string())
        })
        .map(|report| profile_report::Report { totals, ..report });
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            let message = format!("could not read the build profile {trace}: {error}");
            if invocation.output == OutputMode::Json {
                let notice = NoticeEvent {
                    related_command: Some(invocation.command.name().to_owned()),
                    path: Some(trace.to_owned()),
                    ..NoticeEvent::warning(profile_report::CODE_PROFILE_UNREADABLE, message)
                };
                return emit_built(out, notice_event(&notice));
            }
            if invocation.chatty() {
                let _ = writeln!(err, "dx: {message}");
            }
            return Ok(());
        }
    };
    let report_file = profile_report::report_path(workspace);
    let value = report.json();
    let _ = std::fs::write(&report_file, format!("{value}\n"));
    if invocation.output == OutputMode::Json {
        let notice = NoticeEvent {
            related_command: Some(invocation.command.name().to_owned()),
            path: Some(report_file.display().to_string()),
            ..NoticeEvent::info(profile_report::CODE_PROFILE_REPORT, value.to_string())
        };
        return emit_built(out, notice_event(&notice));
    }
    if invocation.chatty() {
        let _ = writeln!(err, "{}", report.text());
    }
    Ok(())
}

fn emit_build_completion(
    invocation: &Invocation,
    out: &mut dyn Write,
    bazel_code: i32,
    code: i32,
    actions: Option<ActionCounts>,
) -> Result<(), i32> {
    if bazel_code != 0 {
        emit_bazel_exit(
            out,
            bazel_failure_code(bazel_code),
            &format!(
                "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                invocation.command.name(),
            ),
            bazel_code,
        )?;
    }
    let counts = FinishedCounts {
        actions,
        ..FinishedCounts::default()
    };
    emit_event(out, &command_finished(code, &counts))?;
    flush_out(out)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    #[test]
    fn workflow_phase_reports_test_completeness() {
        let harness = Harness::new("workflow-phase-build");
        let inv = dx_plan::args::apply_here(
            &invocation(&["build", "--output=text"]),
            &harness.workspace,
            &harness.cwd,
        )
        .expect("resolve workspace");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let phase = super::execute_workflow_phase(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(phase, (0, false, false));

        let harness = Harness::new("workflow-phase-test");
        let uri = write_bep_artifact(
            &harness,
            "test.xml",
            br#"<testsuites><testsuite name="s"><testcase name="passes" classname="c"/></testsuite></testsuites>"#,
        );
        let harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let inv = dx_plan::args::apply_here(
            &invocation(&["test", "--output=text"]),
            &harness.workspace,
            &harness.cwd,
        )
        .expect("resolve workspace");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let phase = super::execute_workflow_phase(&inv, harness.env(&runner, &mut out, &mut err));
        assert_eq!(phase, (0, true, false));
    }

    #[test]
    fn build_server_crash_reports_server_crashed() {
        let harness = Harness {
            bazel_code: 37,
            ..Harness::new("build-crash")
        };
        let (code, out, _) = harness.run(&["build", "--output=json"]);
        assert_eq!(code, 37, "{out}");
        assert!(out.contains("\"code\":\"server_crashed\""), "{out}");
        assert!(out.contains("\"cause\":\"host\""), "{out}");
        assert!(!out.contains("bazel_failed"), "{out}");
    }

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
        for flag in dx_plan::args::AGENT_QUIET_FLAGS {
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
        let inv = dx_plan::args::parse_with(&dx_testing::cli::strings(&["test", "//..."]), &env)
            .expect("parse");
        let harness = Harness::new("agent-done");
        let run = harness.probe_with(&inv, &[Some(0)]);
        for flag in dx_plan::args::AGENT_QUIET_FLAGS {
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
        assert!(harness.workspace.join(".dx/local/results.json").is_file());
    }

    #[test]
    fn profile_passes_the_trace_flag_and_reports_json_and_text() {
        let harness = Harness::new("build-profile");
        let dir = harness.workspace.join(".dx/local").join("timings");
        std::fs::create_dir_all(&dir).expect("dir");
        let id = dx_plan::plan::run_id(std::process::id(), 0, &harness.temp);
        let trace = dir.join(format!("{id}.json.gz"));
        let json =
            r#"{"traceEvents":[{"cat":"action processing","name":"slow","ts":0,"dur":2000000}]}"#;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, json.as_bytes()).expect("gzip");
        std::fs::write(&trace, gz.finish().expect("finish")).expect("trace");
        let inv = invocation(&["build", "--timings", "--output=json"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        let flag = format!("--profile={}", trace.display());
        assert!(run.argv[0].contains(&flag), "{:?}", run.argv);
        assert!(
            run.argv[0]
                .contains(&"--experimental_collect_local_sandbox_action_metrics".to_string()),
            "{:?}",
            run.argv
        );
        let notice = run
            .out
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
            .find(|event| event["code"] == "timings_report")
            .expect("notice");
        assert_eq!(
            notice["path"],
            dir.join("report.json").display().to_string()
        );
        assert!(
            notice["message"]
                .as_str()
                .expect("message")
                .contains("slow")
        );
        assert!(dir.join("report.json").is_file());
        let (code, _, err) = harness.run(&["build", "--timings"]);
        assert_eq!(code, 0);
        assert!(err.contains("dx: timings, wall time 2.0s"), "{err}");
        let run = harness.probe_with(&invocation(&["build"]), &[Some(0)]);
        assert!(!run.argv[0].iter().any(|arg| arg.starts_with("--profile")));
    }

    #[test]
    fn profile_reports_json_and_text_from_the_trace() {
        let harness = Harness::new("build-profile-report");
        let dir = harness.workspace.join(".dx/local").join("timings");
        std::fs::create_dir_all(&dir).expect("dir");
        let trace = dir.join("1.json.gz");
        let json =
            r#"{"traceEvents":[{"cat":"action processing","name":"slow","ts":0,"dur":2000000}]}"#;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, json.as_bytes()).expect("gzip");
        std::fs::write(&trace, gz.finish().expect("finish")).expect("trace");
        let trace = trace.to_str().expect("utf8");
        let bep = harness.workspace.join("missing-bep.json");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let inv = invocation(&["build", "--timings", "--output=json"]);
        super::announce_profile(
            &inv,
            &harness.workspace,
            &mut out,
            &mut err,
            trace,
            Some(&bep),
            None,
        )
        .expect("announce");
        let notice: serde_json::Value = serde_json::from_slice(&out).expect("json");
        assert_eq!(notice["code"], "timings_report");
        assert_eq!(
            notice["path"],
            dir.join("report.json").display().to_string()
        );
        assert!(
            notice["message"]
                .as_str()
                .expect("message")
                .contains("slow")
        );
        assert!(dir.join("report.json").is_file());
        let mut err = Vec::new();
        let inv = invocation(&["build", "--timings"]);
        super::announce_profile(
            &inv,
            &harness.workspace,
            &mut Vec::new(),
            &mut err,
            trace,
            Some(&bep),
            None,
        )
        .expect("announce");
        let err = String::from_utf8(err).expect("utf8");
        assert!(err.contains("dx: timings, wall time 2.0s"), "{err}");
    }

    #[test]
    fn a_users_execution_log_flag_reaches_bazel() {
        let harness = Harness::new("build-own-exec-log");
        let flag = "--execution_log_compact_file=/tmp/l";
        let inv = invocation(&["build", "--", flag]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert!(run.argv[0].contains(&flag.to_owned()), "{:?}", run.argv);
        assert!(!harness.workspace.join(".dx/local/explain").exists());
    }

    #[test]
    fn build_mode_flags_reach_bazel_argv() {
        for (words, flag) in [
            (vec!["build"], "--config=dx_build_dev"),
            (vec!["build", "--debug"], "--config=dx_build_debug"),
            (vec!["build", "--release"], "--config=dx_build_release"),
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
    fn build_forwards_consumer_configs_and_rejects_conflicting_dx_build_mode() {
        let harness = Harness::new("build-consumer-config");
        let inv = invocation(&["build", "--", "--config=ci", "--config=sanitizer"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let argv = &run.argv[0];
        let dev = argv
            .iter()
            .position(|arg| arg == "--config=dx_build_dev")
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
        let (code, _, err) = harness.run(&["build", "--", "--config=dx_build_release"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("config"), "{err}");
    }

    #[test]
    fn test_and_coverage_forward_test_binary_args_to_bazel() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-test-args");
            harness.query.script_stdout("py_test rule //pkg:unit\n");
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
            harness.query.script_stdout(
                "_rust_forward_test rule //cli:dx_cli_test\nrust_test rule //cli:raw_test\nrustfmt_test rule //cli:fmt_test\nrust_clippy_test rule //cli:clippy_test\n",
            );
            harness.query.script_stdout("");
            let inv = invocation(&[command, "--", "--test_filter=slots"]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            let queries = harness.query.calls.borrow();
            assert_eq!(queries.len(), 2, "{command}: {queries:?}");
            assert!(
                harness.query.expressions.borrow()[1].contains("attr(use_libtest_harness, 0,"),
                "{command}: {:?}",
                harness.query.expressions.borrow()
            );
            let query = &queries[0];
            assert!(query.contains(&"query".to_owned()), "{query:?}");
            assert!(
                query.contains(&"--output=label_kind".to_owned()),
                "{query:?}"
            );
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
    fn test_filter_skips_the_libtest_filter_for_tests_without_the_harness() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-rust-no-harness");
            harness
                .query
                .script_stdout("_rust_forward_test rule //cli:raw_test\n");
            harness.query.script_stdout("//cli:raw_test_upstream\n");
            let inv = invocation(&[command, "--", "--test_filter=slots"]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            assert_eq!(harness.query.calls.borrow().len(), 2, "{command}");
            let argv = &run.argv[0];
            assert!(
                !argv.iter().any(|arg| arg.starts_with("--test_arg")),
                "{command}: {argv:?}"
            );
            assert!(argv.contains(&"--test_filter=slots".to_owned()), "{argv:?}");
        }
    }

    #[test]
    fn test_filter_kind_query_failure_stops_before_bazel() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-rust-filter-fails");
            harness.query.script_failure(7, "ERROR: no such package\n");
            let inv = invocation(&[command, "--", "--test_filter=slots"]);
            let mut err = Vec::new();
            let run = harness.probe_streams(&inv, &[Some(0)], &mut Vec::new(), &mut err);
            let err = String::from_utf8(err).expect("stderr");
            assert_ne!(run.code, 0, "{command}: {err}");
            assert!(run.argv.is_empty(), "{command}: {run:?}");
            assert!(err.contains("query"), "{command}: {err}");
        }
    }

    #[test]
    fn test_filter_kind_query_forwards_only_repository_options() {
        for command in ["test", "coverage"] {
            let harness = Harness::new("wf-rust-filter-options");
            harness
                .query
                .script_stdout("_rust_forward_test rule //cli:dx_cli_test\n");
            harness.query.script_stdout("");
            let inv = invocation(&[
                command,
                "--",
                "--test_filter=slots",
                "--config=ci",
                "--override_repository=foo=/x",
                "--repo_env=K=V",
                "--test_tag_filters=unit",
                "--platforms=//:x",
            ]);
            let run = harness.probe_with(&inv, &[Some(0)]);
            let queries = harness.query.calls.borrow();
            assert_eq!(queries.len(), 2, "{command}: {queries:?}");
            for query in queries.iter() {
                for kept in ["--override_repository=foo=/x", "--repo_env=K=V"] {
                    assert!(query.contains(&kept.to_owned()), "{kept}: {query:?}");
                }
                for dropped in [
                    "--test_filter=slots",
                    "--test_tag_filters=unit",
                    "--platforms=//:x",
                    "--config=ci",
                ] {
                    assert!(!query.contains(&dropped.to_owned()), "{dropped}: {query:?}");
                }
                assert!(
                    !query.iter().any(|arg| arg.starts_with("--config=dx_")),
                    "{query:?}"
                );
            }
            let argv = &run.argv[0];
            assert!(argv.contains(&"--config=ci".to_owned()), "{argv:?}");
            assert!(argv.contains(&"--test_arg=slots".to_owned()), "{argv:?}");
        }
    }

    #[test]
    fn test_filter_with_a_conflicting_build_mode_still_exits_usage() {
        let harness = Harness::new("wf-rust-filter-conflict");
        let inv = invocation(&["test", "--", "--test_filter=x", "--config=dx_build_release"]);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(0)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 2, "{err}");
        assert!(run.argv.is_empty(), "{run:?}");
    }

    #[test]
    fn test_filter_stays_a_bazel_option_for_mixed_or_empty_selections() {
        for kinds in [
            "_rust_forward_test rule //cli:unit\npy_test rule //py:unit\n",
            "rustfmt_test rule //cli:fmt_test\n",
            "",
        ] {
            let harness = Harness::new("wf-mixed-filter");
            harness.query.script_stdout(kinds);
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
    fn build_json_reports_action_counts() {
        let mut harness = Harness::new("wf-build-actions");
        let (code, out, _) = harness.run(&["build", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(finished_actions(&out), None, "{out}");
        harness.build_metrics = Some(build_metrics_line(10, 9, 2, 3, 5));
        let (code, out, _) = harness.run(&["build", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            finished_actions(&out),
            Some(
                serde_json::json!({"created": 10, "executed": 4, "cache_hits": {"disk": 2, "remote": 3, "local": 5}})
            )
        );
    }

    #[test]
    fn test_json_reports_action_counts() {
        let harness = Harness::new("wf-test-actions");
        let uri = write_bep_artifact(
            &harness,
            "test.xml",
            br#"<testsuites><testsuite name="s"><testcase name="passes" classname="c"/></testsuite></testsuites>"#,
        );
        let mut harness = Harness {
            raw_bep: Some(vec![test_result_line(
                "//a:t",
                &[(String::from("test.xml"), uri)],
            )]),
            ..harness
        };
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(finished_actions(&out), None, "{out}");
        harness.build_metrics = Some(build_metrics_line(10, 9, 2, 3, 5));
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            finished_actions(&out),
            Some(
                serde_json::json!({"created": 10, "executed": 4, "cache_hits": {"disk": 2, "remote": 3, "local": 5}})
            )
        );
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
    fn workflow_json_removes_stale_default_results_before_an_early_failure() {
        let mut harness = Harness::new("wf-stale-results");
        let stale = harness.workspace.join(".dx/local/results.json");
        std::fs::create_dir_all(stale.parent().expect("parent")).expect("dx dir");
        std::fs::write(&stale, "stale").expect("stale results");
        harness.io_error = true;
        let (code, out, _) = harness.run(&["test", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("launch_failed"), "{out}");
        assert!(!stale.exists(), "stale results must not survive: {out}");
    }

    #[test]
    fn workflow_json_removes_stale_default_results_before_a_report_error() {
        for (name, report_args) in [
            (
                "wf-stale-dup",
                &["--report=junit=a.xml", "--report=junit=a.xml"][..],
            ),
            ("wf-stale-bogus", &["--report=bogus=x"][..]),
        ] {
            let harness = Harness::new(name);
            let stale = harness.workspace.join(".dx/local/results.json");
            std::fs::create_dir_all(stale.parent().expect("parent")).expect("dx dir");
            std::fs::write(&stale, "stale").expect("stale results");
            let mut args = vec!["test", "--output=json"];
            args.extend_from_slice(report_args);
            let (code, out, _) = harness.run(&args);
            assert_eq!(code, 2, "{name}: {out}");
            assert!(
                !stale.exists(),
                "{name}: stale results must not survive: {out}"
            );
        }
    }

    #[test]
    fn workflow_json_keeps_default_results_when_a_results_report_is_requested() {
        let mut harness = Harness::new("wf-keep-results");
        let stale = harness.workspace.join(".dx/local/results.json");
        std::fs::create_dir_all(stale.parent().expect("parent")).expect("dx dir");
        std::fs::write(&stale, "stale").expect("stale results");
        harness.io_error = true;
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=r.json"]);
        assert_eq!(code, 1, "{out}");
        assert!(
            stale.exists(),
            "results report run must keep the file: {out}"
        );
        let (code, out, _) = harness.run(&["test", "--output=json", "--report=results=-"]);
        assert_eq!(code, 2, "{out}");
        assert!(
            stale.exists(),
            "stdout results report must keep the file: {out}"
        );
    }

    #[test]
    fn workflow_json_build_keeps_stale_results_on_a_report_error() {
        let harness = Harness::new("wf-build-keep-results");
        let stale = harness.workspace.join(".dx/local/results.json");
        std::fs::create_dir_all(stale.parent().expect("parent")).expect("dx dir");
        std::fs::write(&stale, "stale").expect("stale results");
        let (code, out, _) = harness.run(&["build", "--output=json", "--report=bogus=x"]);
        assert_eq!(code, 2, "{out}");
        assert!(stale.exists(), "build must keep the file: {out}");
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
        harness
            .query
            .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
        let inv = invocation(&[
            "build",
            "--bazel-startup-option=--output_base=/tmp/a",
            "--bazel-startup-option=--output_user_root=/tmp/b",
            "pkg/a.py",
        ]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 1, "one rule graph query");
        assert_eq!(
            queries[0],
            vec![
                "bazel".to_owned(),
                "--nohome_rc".to_owned(),
                "--nosystem_rc".to_owned(),
                "--output_base=/tmp/a".to_owned(),
                "--output_user_root=/tmp/b".to_owned(),
                "query".to_owned(),
                "--keep_going".to_owned(),
                "--output=streamed_jsonproto".to_owned(),
                "--proto:output_rule_attrs=tags".to_owned(),
                "--noproto:default_values".to_owned(),
                "--noproto:locations".to_owned(),
                "--noproto:instantiation_stack".to_owned(),
                "--".to_owned(),
                "kind('rule', //...)".to_owned(),
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
        harness.query.script_graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            graph_rule("//pkg:unit", &["//pkg:lib"], &[]),
        ]);
        harness.query.script_stdout("//pkg:unit\n");
        harness
            .query
            .script_stdout("//pkg:lib (abc)\n//pkg:unit (abc)\n");
        let inv = invocation(&["test", "pkg/a.py", "--", "--platforms=//:x"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        let queries = harness.query.calls.borrow();
        assert_eq!(queries.len(), 3, "graph plus mapping: {queries:?}");
        assert_eq!(
            queries[0][3], "query",
            "ownership stays unconfigured: {queries:?}"
        );
        assert_eq!(
            queries[2][3], "cquery",
            "test mapping uses execution configuration: {queries:?}"
        );
        assert_eq!(
            harness.query.expressions.borrow()[2],
            "rdeps(set(\"//pkg:unit\"), set(\"//pkg:lib\"))"
        );
        assert!(
            queries[2].contains(&"--platforms=//:x".to_owned()),
            "mapping shares platforms with execution: {queries:?}"
        );
        assert!(
            queries[2].contains(&"--config=dx_build_dev".to_owned()),
            "mapping shares profile with execution: {queries:?}"
        );
        for query in &queries[..2] {
            for dropped in ["--platforms=//:x", "--config=dx_build_dev"] {
                assert!(
                    !query.contains(&dropped.to_owned()),
                    "unconfigured queries ignore configuration: {query:?}"
                );
            }
        }
        assert_eq!(run.argv.len(), 1, "{run:?}");
        let argv = &run.argv[0];
        assert!(argv.contains(&"--platforms=//:x".to_owned()), "{argv:?}");
        assert!(
            argv.contains(&"--config=dx_build_dev".to_owned()),
            "{argv:?}"
        );
    }

    #[test]
    fn test_file_scope_conflicting_build_mode_is_pre_exec() {
        let harness = Harness::new("wf-test-conflict");
        harness.write_source("pkg/BUILD.bazel", "");
        harness.write_source("pkg/a.py", "x = 1\n");
        harness
            .query
            .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
        let (code, _, err) = harness.run(&["test", "pkg/a.py", "--", "--config=dx_build_release"]);
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
#[path = "build_slots_launcher_tests.rs"]
mod build_slots_launcher_tests;

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
