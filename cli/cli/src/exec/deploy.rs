use super::common::*;
use crate::args::{resolve_profile, Command, Invocation, Profile, DX_PROFILE_ENV};
use crate::plan::{plan_deploy_build, plan_deploy_run};
use crate::reports::plan_reports;
use crate::resolve::{check_deployable, resolve_deploy};
use dx_output::{command_finished, command_started, write_event, FinishedCounts, OutputMode};

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
    debug_assert!(planned_reports.is_empty(), "dx deploy takes no --report");
    let label = match resolve_deploy(&invocation.targets) {
        Ok(label) => label,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let info = match check_deployable(&label, workspace, query_runner) {
        Ok(info) => info,
        Err(error) => return pre_exec(err, &error.to_string()),
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
    let build_plan = plan_deploy_build(&label, profile);
    let run_plan = plan_deploy_run(&label, &invocation.bazel_options, profile);
    let app_display = if info.app_raw == "NONE" || info.app_raw == "None" {
        label.clone()
    } else {
        info.app_raw.clone()
    };
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            let _ = write_event(out, &command_finished(0, &FinishedCounts::default()));
        } else if !invocation.quiet {
            let _ = writeln!(
                err,
                "Deploy {label} (app: {app_display}, profile: {})",
                profile.name()
            );
            let _ = writeln!(err, "build: {}", build_plan.argv.join(" "));
            let _ = writeln!(err, "run: {}", run_plan.argv.join(" "));
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
    match run_bazel(
        invocation,
        out,
        err,
        workspace,
        runner,
        &run_plan.argv,
        &[(DX_PROFILE_ENV, profile_name)],
    ) {
        Ok(code) => code,
        Err(exit) => exit,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::args::parse;
    use crate::exec::execute;
    use crate::resolve::QueryResult;
    use dx_process::{ChildStatus, Runner};
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;

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
        let (code, _, err) = harness.run(&["deploy", "//app:bin"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Running deploy for //app:bin"), "{err}");
        assert!(err.contains("profile release"), "{err}");
    }

    #[test]
    fn deploy_dry_run_prints_plan_without_exec() {
        let harness = harness_with_deploy("deploy-dry", "True|release|None|True");
        let (code, _, err) = harness.run(&["deploy", "//deploy:prod", "--dry-run"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("Deploy //deploy:prod"), "{err}");
        assert!(err.contains("profile: release"), "{err}");
        assert!(err.contains("build:"), "{err}");
        assert!(err.contains("run:"), "{err}");
        assert!(err.contains("--config=dx_release"), "{err}");
        assert_eq!(harness.query.calls.borrow().len(), 1, "one cquery");
    }

    #[test]
    fn deploy_flag_over_attr_precedence() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let harness = harness_with_deploy("deploy-prec", "True|debug|None|True");
        let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>> = Rc::new(RefCell::new(Vec::new()));
        let probe = RecordingRunner {
            code: Some(0),
            seen: Rc::clone(&seen),
            seen_env: Rc::clone(&seen_env),
        };
        let inv = invocation(&["deploy", "--release", "//deploy:prod"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(&inv, harness.env(&probe, &mut out, &mut err));
        assert_eq!(code, 0);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2, "build then run");
        assert!(
            seen[0].contains(&"--config=dx_release".to_owned()),
            "{seen:?}"
        );
        assert!(
            seen[1].contains(&"--config=dx_release".to_owned()),
            "{seen:?}"
        );
        let seen_env = seen_env.borrow();
        assert_eq!(seen_env.len(), 2);
        assert!(
            seen_env[1].contains(&("DX_PROFILE".to_owned(), "release".to_owned())),
            "{seen_env:?}"
        );
        let harness = harness_with_deploy("deploy-attr", "True|debug|None|True");
        let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let seen_env2: Rc<RefCell<Vec<Vec<(String, String)>>>> = Rc::new(RefCell::new(Vec::new()));
        let probe = RecordingRunner {
            code: Some(0),
            seen: Rc::clone(&seen),
            seen_env: Rc::clone(&seen_env2),
        };
        let inv = invocation(&["deploy", "//deploy:prod"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(&inv, harness.env(&probe, &mut out, &mut err));
        assert_eq!(code, 0);
        let seen = seen.borrow();
        assert!(
            seen[0].contains(&"--config=dx_debug".to_owned()),
            "{seen:?}"
        );
    }

    #[test]
    fn deploy_preserves_exit_codes_and_forwards_args() {
        let harness = harness_with_deploy("deploy-buildfail", "True|release|None|True");
        let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let probe = FailFirstRunner {
            seen: Rc::clone(&seen),
        };
        let inv = invocation(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(&inv, harness.env(&probe, &mut out, &mut err));
        assert_eq!(code, 3);
        assert_eq!(
            seen.borrow().len(),
            1,
            "run never launches after build failure"
        );
        let harness = harness_with_deploy("deploy-runfail", "True|release|None|True");
        let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let probe = SucceedBuildFailRun {
            code: 7,
            seen: Rc::clone(&seen),
        };
        let inv = invocation(&["deploy", "//deploy:prod", "--", "--port=8080"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(&inv, harness.env(&probe, &mut out, &mut err));
        assert_eq!(code, 7);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert!(seen[1].contains(&"--".to_owned()), "{seen:?}");
        assert!(seen[1].contains(&"--port=8080".to_owned()), "{seen:?}");
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

    struct RecordingRunner {
        code: Option<i32>,
        seen: Rc<RefCell<Vec<Vec<String>>>>,
        seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    }

    impl Runner for RecordingRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &std::path::Path,
            env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            self.seen.borrow_mut().push(argv.to_vec());
            self.seen_env.borrow_mut().push(
                env.iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            );
            Ok(ChildStatus { code: self.code })
        }
    }

    struct FailFirstRunner {
        seen: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Runner for FailFirstRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &std::path::Path,
            _env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            self.seen.borrow_mut().push(argv.to_vec());
            Ok(ChildStatus { code: Some(3) })
        }
    }

    struct SucceedBuildFailRun {
        code: i32,
        seen: Rc<RefCell<Vec<Vec<String>>>>,
    }

    impl Runner for SucceedBuildFailRun {
        fn run(
            &self,
            argv: &[String],
            _cwd: &std::path::Path,
            _env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            let n = self.seen.borrow().len();
            self.seen.borrow_mut().push(argv.to_vec());
            if n == 0 {
                Ok(ChildStatus { code: Some(0) })
            } else {
                Ok(ChildStatus {
                    code: Some(self.code),
                })
            }
        }
    }
}
