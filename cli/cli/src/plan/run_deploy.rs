use super::BuildPlan;

pub fn plan_run_targets(
    targets: &[String],
    app_args: &[String],
    profile: crate::args::Profile,
) -> BuildPlan {
    use dx_process::{launcher_argv0, WORKFLOW_STARTUP_OPTS};

    let mut argv =
        Vec::with_capacity(WORKFLOW_STARTUP_OPTS.len() + 4 + targets.len() + app_args.len());
    argv.push(launcher_argv0().to_owned());
    argv.extend(WORKFLOW_STARTUP_OPTS.iter().map(ToString::to_string));
    argv.push("run".to_owned());
    argv.push(profile.config_flag());
    argv.extend(targets.iter().cloned());
    if !app_args.is_empty() {
        argv.push("--".to_owned());
        argv.extend(app_args.iter().cloned());
    }
    let summary = format!("Running run for {}", targets.join(" "));
    BuildPlan { argv, summary }
}

pub fn plan_run(target: &str, app_args: &[String], profile: crate::args::Profile) -> BuildPlan {
    plan_run_targets(&[target.to_owned()], app_args, profile)
}

pub fn plan_deploy_build(label: &str, profile: crate::args::Profile) -> BuildPlan {
    use dx_process::{launcher_argv0, WORKFLOW_STARTUP_OPTS};

    let mut argv = Vec::with_capacity(WORKFLOW_STARTUP_OPTS.len() + 4);
    argv.push(launcher_argv0().to_owned());
    argv.extend(WORKFLOW_STARTUP_OPTS.iter().map(ToString::to_string));
    argv.push("build".to_owned());
    argv.push(profile.config_flag());
    argv.push(label.to_owned());
    let summary = format!("Running deploy build for {label}");
    BuildPlan { argv, summary }
}

pub fn plan_deploy_run(
    label: &str,
    app_args: &[String],
    profile: crate::args::Profile,
) -> BuildPlan {
    let plan = plan_run(label, app_args, profile);
    let summary = format!("Running deploy run for {label}");
    BuildPlan {
        argv: plan.argv,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Profile;
    use crate::test_support::strings;

    #[test]
    fn run_targets_share_single_builder() {
        let single = plan_run("//app:bin", &strings(&["--port=8080"]), Profile::Dev);
        let multi = plan_run_targets(
            &strings(&["//app:bin"]),
            &strings(&["--port=8080"]),
            Profile::Dev,
        );
        assert_eq!(single, multi);
        let joined = plan_run_targets(&strings(&["//a:one", "//b:two"]), &[], Profile::Dev);
        assert_eq!(
            joined.argv.last(),
            Some(&"//b:two".to_owned()),
            "{joined:?}"
        );
        assert!(joined.summary.contains("//a:one //b:two"));
    }

    #[test]
    fn run_plan_forwards_app_args_verbatim() {
        let plan = plan_run("//app:bin", &strings(&["--port=8080"]), Profile::Dev);
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--config=dx_dev",
                "//app:bin",
                "--",
                "--port=8080",
            ])
        );
        assert!(plan.summary.contains("//app:bin"));
        let bare = plan_run("//app:bin", &[], Profile::Dev);
        assert!(!bare.argv.contains(&"--".to_owned()));
    }

    #[test]
    fn run_profile_pins_config_flag() {
        for (profile, flag) in [
            (Profile::Debug, "--config=dx_debug"),
            (Profile::Dev, "--config=dx_dev"),
            (Profile::Release, "--config=dx_release"),
        ] {
            let plan = plan_run("//app:bin", &[], profile);
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            assert_eq!(
                argv[..5],
                ["bazel", "--nohome_rc", "--nosystem_rc", "run", flag,],
                "{profile:?}: {plan:?}"
            );
        }
    }
}
