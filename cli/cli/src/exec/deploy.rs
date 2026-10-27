use super::common::*;
use dx_cli_core::args::{resolve_profile, Command, Invocation, Profile, DX_PROFILE_ENV};
use dx_cli_core::plan::{plan_deploy_build, plan_deploy_run, shell_join};
use dx_cli_core::reports::plan_reports;
use dx_cli_core::resolve::{check_deployable, resolve_deploy};

pub(crate) fn execute_deploy(invocation: &Invocation, env: Env<'_>) -> i32 {
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
    debug_assert!(planned_reports.is_empty(), "dx deploy takes no --report");
    let label = match resolve_deploy(&invocation.targets) {
        Ok(label) => label,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let info = match check_deployable(
        &label,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(info) => info,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let attr = if info.profile_raw == "NONE" || info.profile_raw == "None" {
        None
    } else {
        Profile::parse_attr(&info.profile_raw)
    };
    let profile = resolve_profile(
        invocation.profile_flag(),
        attr,
        Profile::default_for(Command::Deploy),
    );
    let plans =
        plan_deploy_build(&label, profile, &invocation.bazel_startup_options).and_then(|build| {
            plan_deploy_run(
                &label,
                invocation.raw_bazel_options(),
                profile,
                &invocation.bazel_startup_options,
            )
            .map(|run| (build, run))
        });
    let (build_plan, run_plan) = match plans {
        Ok(plans) => plans,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    if !invocation.applies() {
        if !invocation.quiet {
            let _ = writeln!(err, "{}", build_plan.summary);
        }
        let build_code = match run_bazel(
            invocation,
            out,
            err,
            workspace,
            runner,
            &build_plan.argv,
            &[],
        ) {
            Ok(code) => code,
            Err(exit) => return exit,
        };
        if build_code != 0 {
            return build_code;
        }
        if !invocation.quiet {
            let mut words = vec!["dx".to_owned(), "deploy".to_owned(), "--apply".to_owned()];
            if invocation.debug {
                words.push("--debug".to_owned());
            }
            if invocation.release {
                words.push("--release".to_owned());
            }
            words.push(label.clone());
            if !invocation.raw_bazel_options().is_empty() {
                let (safe, withheld) = dx_cli_core::plan::quality::partition_provenance_options(
                    invocation.raw_bazel_options(),
                );
                words.push("--".to_owned());
                words.extend(safe);
                words.extend(withheld);
            }
            let _ = writeln!(
                err,
                "Validated {label} (built, not published; publish with: {})",
                shell_join(&words)
            );
        }
        return 0;
    }
    if !invocation.quiet {
        let _ = writeln!(
            err,
            "Running deploy for {label} (profile {})",
            profile.name()
        );
    }
    let build_code = match run_bazel(
        invocation,
        out,
        err,
        workspace,
        runner,
        &build_plan.argv,
        &[],
    ) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if build_code != 0 {
        return build_code;
    }
    let profile_name = profile.name();
    match run_bazel_app(
        invocation,
        out,
        err,
        workspace,
        runner,
        &run_plan.argv,
        &[(DX_PROFILE_ENV, profile_name)],
    ) {
        Ok(AppExit {
            code,
            signal: Some(signo),
        }) => {
            app_signalled_line(err, &label, signo);
            code
        }
        Ok(app) => app.code,
        Err(exit) => exit,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_cli_core::args::parse;
    use dx_cli_core::resolve::QueryResult;

    fn deploy_query_output(output: &str) -> QueryResult {
        QueryResult {
            code: Some(0),
            stdout: output.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn harness_with_deploy(name: &str, cquery_stdout: &str) -> Harness {
        let harness = Harness::new(name);
        harness
            .query
            .outputs
            .borrow_mut()
            .push(deploy_query_output(cquery_stdout));
        harness
    }

    #[test]
    fn deploy_empty_and_multiple_are_pre_exec() {
        let harness = Harness::new("deploy-count-empty");
        let (code, _, err) = harness.run(&["deploy"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("exactly one label"), "{err}");
        let harness = Harness::new("deploy-count-multi");
        let (code, _, err) = harness.run(&["deploy", "//a:one", "//b:two"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("exactly one label"), "{err}");
    }

    #[test]
    fn deploy_pattern_and_path_are_pre_exec() {
        let harness = Harness::new("deploy-pattern");
        let (code, _, err) = harness.run(&["deploy", "//..."]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("pass exactly one deploy label"), "{err}");
        let harness = Harness::new("deploy-path");
        harness.write_source("pkg/a.py", "x = 1\n");
        let (code, _, err) = harness.run(&["deploy", "pkg/a.py"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("pass exactly one deploy label"), "{err}");
    }

    #[test]
    fn deploy_not_deployable_is_pre_exec() {
        let harness = harness_with_deploy("deploy-not", "False|NONE|NONE|False");
        let (code, _, err) = harness.run(&["deploy", "//pkg:lib"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("not_deployable"), "{err}");
    }

    #[test]
    fn deploy_executable_without_provider_runs() {
        let harness = harness_with_deploy("deploy-exe", "False|NONE|NONE|True");
        let (code, _, err) = harness.run(&["deploy", "--apply", "//app:bin"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running deploy for //app:bin"), "{err}");
        assert!(err.contains("profile release"), "{err}");
    }

    #[test]
    fn deploy_signalled_app_exits_128_plus_the_signal() {
        let mut harness = harness_with_deploy("deploy-app-signal", "False|NONE|NONE|True");
        harness.app_signal = Some(11);
        let (code, _, err) = harness.run(&["deploy", "--apply", "//app:bin"]);
        assert_eq!(code, 139, "{err}");
        assert!(
            err.contains("dx: application_failed: bazel run //app:bin terminated by signal 11"),
            "{err}"
        );
        assert!(!err.contains("bazel_signalled"), "{err}");
    }

    #[test]
    fn deploy_check_builds_without_publishing() {
        let harness = harness_with_deploy("deploy-check", "True|release|None|True");
        let (code, _, err) = harness.run(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("Running deploy build for //deploy:prod"),
            "{err}"
        );
        assert!(
            err.contains("dx deploy --apply //deploy:prod -- --port=8080"),
            "{err}"
        );
        assert!(!err.contains("Running deploy for //deploy:prod"), "{err}");
        let harness = harness_with_deploy("deploy-check-probe", "True|release|None|True");
        let inv = invocation(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(
            run.argv.len(),
            1,
            "build only, the deploy step never launches: {run:?}"
        );
        assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
        assert!(
            !run.argv[0].contains(&"--port=8080".to_owned()),
            "app args stay out of the validation build: {run:?}"
        );
    }

    #[test]
    fn deploy_check_reports_build_failures() {
        let harness = harness_with_deploy("deploy-check-fail", "True|release|None|True");
        let inv = invocation(&["deploy", "//deploy:prod"]);
        let run = harness.probe_with(&inv, &[Some(4)]);
        assert_eq!(run.code, 4, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
    }

    #[test]
    fn deploy_flag_over_attr_precedence() {
        let harness = harness_with_deploy("deploy-prec", "True|debug|None|True");
        let inv = invocation(&["deploy", "--apply", "--release", "//deploy:prod"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        assert_eq!(run.argv.len(), 2, "build then run");
        let calls = harness.query.calls.borrow();
        let cquery = &calls[0];
        assert!(cquery.contains(&"cquery".to_owned()), "{cquery:?}");
        assert_eq!(
            cquery.last(),
            Some(&"//deploy:prod".to_owned()),
            "{cquery:?}"
        );
        for setting in dx_cli_core::plan::BAZEL_SETTINGS {
            assert!(cquery.contains(&setting.to_owned()), "{cquery:?}");
        }
        for argv in &run.argv {
            for setting in dx_cli_core::plan::BAZEL_SETTINGS {
                assert!(argv.contains(&setting.to_owned()), "{argv:?}");
            }
        }
        assert!(
            run.argv[0].contains(&"--config=dx_release".to_owned()),
            "{run:?}"
        );
        assert!(
            run.argv[1].contains(&"--config=dx_release".to_owned()),
            "{run:?}"
        );
        let seen_env = harness.seen_env.borrow();
        assert_eq!(seen_env.len(), 2);
        assert!(
            seen_env[1].contains(&("DX_PROFILE".to_owned(), "release".to_owned())),
            "{seen_env:?}"
        );
        let harness = harness_with_deploy("deploy-attr", "True|debug|None|True");
        let inv = invocation(&["deploy", "--apply", "//deploy:prod"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        assert!(
            run.argv[0].contains(&"--config=dx_debug".to_owned()),
            "{run:?}"
        );
    }

    #[test]
    fn deploy_preserves_exit_codes_and_forwards_args() {
        let harness = harness_with_deploy("deploy-buildfail", "True|release|None|True");
        let inv = invocation(&["deploy", "--apply", "//deploy:prod", "--", "--port=8080"]);
        let run = harness.probe_with(&inv, &[Some(3)]);
        assert_eq!(run.code, 3);
        assert_eq!(run.argv.len(), 1, "run never launches after build failure");
        let harness = harness_with_deploy("deploy-runfail", "True|release|None|True");
        let inv = invocation(&["deploy", "--apply", "//deploy:prod", "--", "--port=8080"]);
        let run = harness.probe_with(&inv, &[Some(0), Some(7)]);
        assert_eq!(run.code, 7);
        assert_eq!(run.argv.len(), 2, "{run:?}");
        assert!(run.argv[1].contains(&"--".to_owned()), "{run:?}");
        assert!(run.argv[1].contains(&"--port=8080".to_owned()), "{run:?}");
    }

    #[test]
    fn deploy_app_exit_36_on_a_nearly_full_disk_keeps_the_code() {
        let mut harness = harness_with_deploy("deploy-app-36", "True|release|None|True");
        harness.free_disk_bytes = Some(1 << 20);
        let inv = invocation(&["deploy", "--apply", "//deploy:prod"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(0), Some(36)], &mut out, &mut err);
        let (out, err) = (
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        );
        assert_eq!(run.code, 36, "{err}");
        assert_eq!(run.argv.len(), 2, "{run:?}");
        assert!(!out.contains("disk_full"), "{out}");
        assert!(!err.contains("disk_full"), "{err}");
        let mut harness = harness_with_deploy("deploy-build-36", "True|release|None|True");
        harness.free_disk_bytes = Some(1 << 20);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(36)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 36, "{err}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        assert!(err.contains("dx: disk_full: "), "{err}");
    }

    #[test]
    fn deploy_bad_report_and_json_are_pre_exec() {
        let args: Vec<String> = ["deploy", "//a:bin", "--report=sarif=a.sarif"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let err = parse(&args).expect_err("deploy report must fail parse");
        assert!(err.to_string().contains("--report"), "{err:?}");
        let args: Vec<String> = ["deploy", "//a:bin", "--output=json"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let err = parse(&args).expect_err("deploy json must fail parse");
        assert!(err.to_string().contains("--output"), "{err:?}");
    }
}
