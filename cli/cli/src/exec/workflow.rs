use super::common::*;
use super::deploy::execute_deploy;
use super::run::execute_run;
use super::test_reports::{execute_test_reports, TestReportsRequest};
use crate::args::{Command, Invocation};
use crate::plan::{bep_path, plan_workflow, WorkflowVerb};
use crate::reports::{plan_reports, Destination};
use crate::resolve::{resolve, resolve_for_test};
use dx_output::{
    command_finished, command_started, error_event, write_event, FinishedCounts, OutputMode,
};
use dx_process::ForwardError;

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
        ci: _,
    } = env;
    let planned_reports = match plan_reports(
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let stdout_report = planned_reports
        .iter()
        .any(|report| report.destination == Destination::Stdout);
    let resolved = match invocation.command {
        Command::Build => resolve(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ),
        Command::Test | Command::Coverage => resolve_for_test(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        ),
        _ => {
            return pre_exec(
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
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let Some(verb) = invocation.command.workflow_verb() else {
        return pre_exec(
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
    let bep_arg = if verb.collects_reports() {
        Some(bep_text.as_str())
    } else {
        None
    };
    let profile = if verb == WorkflowVerb::Coverage {
        None
    } else {
        Some(invocation.profile())
    };
    let plan = match plan_workflow(
        verb,
        &resolved,
        &invocation.bazel_options,
        bep_arg,
        profile,
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &format!("{error}")),
    };
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if invocation.chatty() && !stdout_report {
            let _ = writeln!(out, "{}", plan.summary);
        }
        return 0;
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
    } else if invocation.chatty() && !stdout_report {
        let _ = writeln!(out, "{}", plan.summary);
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &plan.argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if verb == WorkflowVerb::Build {
        if invocation.output == OutputMode::Json {
            if bazel_code != 0 {
                if let Ok(event) = error_event(
                    "bazel_failed",
                    &format!(
                        "Bazel {} failed with exit {bazel_code} (see stderr diagnostics; run `dx status` for toolchain/pin)",
                        invocation.command.name(),
                    ),
                    None,
                    None,
                    Some("execute"),
                ) {
                    let _ = write_event(out, &event);
                }
            }
            let _ = write_event(
                out,
                &command_finished(bazel_code, &FinishedCounts::default()),
            );
        }
        return bazel_code;
    }
    execute_test_reports(TestReportsRequest {
        invocation,
        workspace,
        out,
        err,
        verb,
        bep: &bep,
        planned_reports: &planned_reports,
        stdout_report,
        bazel_code,
        query_runner,
        pid,
        nonce,
    })
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
    fn workflow_dry_run_text_and_json() {
        let harness = Harness::new("wf-dry-text");
        let (code, out, _) = harness.run(&["build", "--dry-run", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("Running build"), "{out}");
        let harness = Harness::new("wf-dry-json");
        let (code, out, _) = harness.run(&["build", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("command_started"), "{out}");
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
}
