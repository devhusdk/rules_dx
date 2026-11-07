use super::common::*;
use dx_plan::args::{Command, DX_BUILD_MODE_ENV, Invocation, Profile, resolve_profile};
use dx_plan::plan::{plan_deploy_build, plan_deploy_run, shell_join};
use dx_plan::reports::plan_reports;
use dx_plan::resolve::{check_deployable, resolve_deploy};
use std::io::Write;

/// Tells the deploy launcher to publish (1) or dry run (0).
const DEPLOY_LIVE_ENV: &str = "DX_DEPLOY_LIVE";

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
        ci,
        audit_today: _,
        actions: _,
        proc_root: _,
        edit_commit_json: _,
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
    if let Some(code) = refuse_agent_apply(invocation, ci, &label, out, err) {
        return code;
    }
    let info = match check_deployable(
        &label,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(info) => info,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let attr = if info.build_mode_raw == "NONE" || info.build_mode_raw == "None" {
        None
    } else {
        Profile::parse_attr(&info.build_mode_raw)
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
                invocation.app_args(),
                profile,
                &invocation.bazel_startup_options,
            )
            .map(|run| (build, run))
        });
    let (build_plan, run_plan) = match plans {
        Ok(plans) => plans,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    if !invocation.quiet {
        let summary = if invocation.applies() {
            format!("Running deploy for {label} (profile {})", profile.name())
        } else {
            build_plan.summary.clone()
        };
        let _ = writeln!(err, "{summary}");
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
    if !invocation.applies() && !invocation.quiet {
        if !info.size_budget.is_empty() {
            let _ = writeln!(
                err,
                "Size of {label}: {}",
                size_line(workspace, &info.size_stamp, &info.size_budget)
            );
        }
        if !info.sbom.is_empty() {
            let _ = writeln!(err, "SBOM of {label}: {}", sbom_line(workspace, &info.sbom));
        }
    }
    if !invocation.applies() && !info.has_provider {
        if !invocation.quiet {
            let _ = writeln!(
                err,
                "Validated {label} (built, not published; publish with: {})",
                publish_command(invocation, &label)
            );
        }
        return 0;
    }
    let live = if invocation.applies() { "1" } else { "0" };
    let code = match run_bazel_app(
        invocation,
        out,
        err,
        workspace,
        runner,
        &run_plan.argv,
        &[(DX_BUILD_MODE_ENV, profile.name()), (DEPLOY_LIVE_ENV, live)],
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
    };
    if code == 0 && !invocation.applies() && !invocation.quiet {
        let _ = writeln!(
            err,
            "Dry run of {label} (nothing published; publish with: {})",
            publish_command(invocation, &label)
        );
    }
    code
}

/// Refuses `dx deploy --apply` in an agent session outside CI; a human or CI publishes.
fn refuse_agent_apply(
    invocation: &Invocation,
    ci: bool,
    label: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    if !invocation.applies() || !invocation.agent || ci {
        return None;
    }
    let message = format!(
        "dx deploy --apply publishes {label}, so a human or CI must run it; dry run with: dx deploy {label}"
    );
    Some(report_failure(
        invocation.output,
        out,
        err,
        &Failure::new(CODE_DEPLOY_NEEDS_HUMAN, &message, 1).without_usage(),
    ))
}

fn publish_command(invocation: &Invocation, label: &str) -> String {
    let mut words = vec!["dx".to_owned(), "deploy".to_owned(), "--apply".to_owned()];
    if invocation.debug {
        words.push("--debug".to_owned());
    }
    if invocation.release {
        words.push("--release".to_owned());
    }
    words.push(label.to_owned());
    if !invocation.app_args().is_empty() {
        let (safe, withheld) =
            dx_plan::plan::quality::partition_provenance_options(invocation.app_args());
        words.push("--".to_owned());
        words.extend(safe);
        words.extend(withheld);
    }
    shell_join(&words)
}

fn sbom_line(workspace: &std::path::Path, sbom: &str) -> String {
    let base = sbom.rsplit('/').next().unwrap_or(sbom);
    match dx_sbom::spdx_package_count(&workspace.join(sbom)) {
        Some(count) => format!("{base} {count} packages"),
        None => format!("{base} is not a readable SPDX document"),
    }
}

fn size_line(workspace: &std::path::Path, stamp: &str, budget: &str) -> String {
    let checked = (!stamp.is_empty())
        .then(|| std::fs::read_to_string(workspace.join(stamp)).ok())
        .flatten();
    match checked {
        Some(text) => format!("{} (size_budget {budget})", text.trim()),
        None => format!("size_budget {budget}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_plan::args::parse;
    use dx_plan::resolve::QueryResult;

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
    fn deploy_without_apply_runs_the_dry_run() {
        let harness = harness_with_deploy("deploy-check", "True|release|None|True");
        let (code, _, err) = harness.run(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("Running deploy build for //deploy:prod"),
            "{err}"
        );
        assert!(
            err.contains("Dry run of //deploy:prod (nothing published; publish with: dx deploy --apply //deploy:prod -- --port=8080)"),
            "{err}"
        );
        let harness = harness_with_deploy("deploy-check-probe", "True|release|None|True");
        let inv = invocation(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        let run = harness.probe_with(&inv, &[Some(0), Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 2, "build then dry run: {run:?}");
        assert!(run.argv[0].contains(&"build".to_owned()), "{run:?}");
        assert!(
            !run.argv[0].contains(&"--port=8080".to_owned()),
            "app args stay out of the validation build: {run:?}"
        );
        assert!(run.argv[1].contains(&"--port=8080".to_owned()), "{run:?}");
        let seen_env = harness.seen_env.borrow();
        assert!(
            seen_env[1].contains(&("DX_DEPLOY_LIVE".to_owned(), "0".to_owned())),
            "{seen_env:?}"
        );
    }

    #[test]
    fn deploy_without_apply_only_builds_a_plain_executable() {
        let harness = harness_with_deploy("deploy-check-exe", "False|NONE|NONE|True");
        let inv = invocation(&["deploy", "//app:bin"]);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(0)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 0, "{err}");
        assert_eq!(
            run.argv.len(),
            1,
            "a plain executable never runs without --apply: {run:?}"
        );
        assert!(
            err.contains("Validated //app:bin (built, not published; publish with: dx deploy --apply //app:bin)"),
            "{err}"
        );
    }

    #[test]
    fn deploy_dry_run_failure_keeps_the_code_and_skips_the_hint() {
        let harness = harness_with_deploy("deploy-check-runfail", "True|release|None|True");
        let inv = invocation(&["deploy", "//deploy:prod"]);
        let mut err = Vec::new();
        let run = harness.probe_streams(&inv, &[Some(0), Some(5)], &mut Vec::new(), &mut err);
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(run.code, 5, "{err}");
        assert!(!err.contains("Dry run of"), "{err}");
    }

    #[test]
    fn deploy_apply_in_an_agent_session_needs_a_human() {
        let harness = harness_with_deploy("deploy-agent", "True|release|None|True");
        let mut inv = invocation(&["deploy", "--apply", "//deploy:prod"]);
        inv.agent = true;
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = super::execute_deploy(&inv, harness.env(&runner, &mut out, &mut err));
        let err = String::from_utf8(err).expect("stderr");
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("deploy_needs_human"), "{err}");
        assert!(err.contains("dx deploy //deploy:prod"), "{err}");
        assert!(harness.seen_env.borrow().is_empty(), "nothing ran");
        let harness = harness_with_deploy("deploy-agent-ci", "True|release|None|True");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut env = harness.env(&runner, &mut out, &mut err);
        env.ci = true;
        let code = super::execute_deploy(&inv, env);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        let harness = harness_with_deploy("deploy-agent-dry", "True|release|None|True");
        let mut inv = invocation(&["deploy", "//deploy:prod"]);
        inv.agent = true;
        let run = harness.probe_with(&inv, &[Some(0), Some(0)]);
        assert_eq!(run.code, 0, "an agent may dry run: {run:?}");
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
        for setting in dx_plan::plan::BAZEL_SETTINGS {
            assert!(cquery.contains(&setting.to_owned()), "{cquery:?}");
        }
        for argv in &run.argv {
            for setting in dx_plan::plan::BAZEL_SETTINGS {
                assert!(argv.contains(&setting.to_owned()), "{argv:?}");
            }
        }
        assert!(
            run.argv[0].contains(&"--config=dx_build_release".to_owned()),
            "{run:?}"
        );
        assert!(
            run.argv[1].contains(&"--config=dx_build_release".to_owned()),
            "{run:?}"
        );
        let seen_env = harness.seen_env.borrow();
        assert_eq!(seen_env.len(), 2);
        assert!(
            seen_env[1].contains(&("DX_BUILD_MODE".to_owned(), "release".to_owned())),
            "{seen_env:?}"
        );
        assert!(
            seen_env[1].contains(&("DX_DEPLOY_LIVE".to_owned(), "1".to_owned())),
            "{seen_env:?}"
        );
        let harness = harness_with_deploy("deploy-attr", "True|debug|None|True");
        let inv = invocation(&["deploy", "--apply", "//deploy:prod"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0);
        assert!(
            run.argv[0].contains(&"--config=dx_build_debug".to_owned()),
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

    #[test]
    fn deploy_check_prints_the_sbom_package_count() {
        let harness = harness_with_deploy(
            "deploy-check-sbom",
            "True|release|None|True|||bazel-out/bin/d/app.spdx.json",
        );
        harness.write_source(
            "bazel-out/bin/d/app.spdx.json",
            r#"{"packages":[{"name":"a"},{"name":"b"}]}"#,
        );
        let (code, _, err) = harness.run(&["deploy", "//deploy:prod"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("SBOM of //deploy:prod: app.spdx.json 2 packages"),
            "{err}"
        );
        let harness = harness_with_deploy(
            "deploy-check-badsbom",
            "True|release|None|True|||bazel-out/none/app.spdx.json",
        );
        let (_, _, err) = harness.run(&["deploy", "//deploy:prod"]);
        assert!(
            err.contains("SBOM of //deploy:prod: app.spdx.json is not a readable SPDX document"),
            "{err}"
        );
        let harness =
            harness_with_deploy("deploy-check-nosbom", "True|release|None|True|1MB|stamp");
        let (_, _, err) = harness.run(&["deploy", "//deploy:prod"]);
        assert!(!err.contains("SBOM of"), "{err}");
    }

    #[test]
    fn deploy_check_prints_the_size_and_budget() {
        let harness = harness_with_deploy(
            "deploy-check-size",
            "True|release|None|True|1MB|bazel-out/bin/deploy/prod.size_budget",
        );
        harness.write_source(
            "bazel-out/bin/deploy/prod.size_budget",
            "2000000 of 1000000 bytes\n",
        );
        let (code, _, err) = harness.run(&["deploy", "//deploy:prod"]);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("Size of //deploy:prod: 2000000 of 1000000 bytes (size_budget 1MB)"),
            "{err}"
        );
        let harness = harness_with_deploy("deploy-check-nosize", "True|release|None|True");
        let (_, _, err) = harness.run(&["deploy", "//deploy:prod"]);
        assert!(!err.contains("Size of"), "{err}");
    }
}
