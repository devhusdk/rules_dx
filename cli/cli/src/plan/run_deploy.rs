use super::BuildPlan;

pub fn plan_run_targets(
    targets: &[String],
    app_args: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> BuildPlan {
    let mut argv = dx_process::startup_argv(startup_options);
    argv.reserve(3 + targets.len() + app_args.len());
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

pub fn plan_run(
    target: &str,
    app_args: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> BuildPlan {
    plan_run_targets(&[target.to_owned()], app_args, profile, startup_options)
}

pub fn plan_run_build(
    targets: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> BuildPlan {
    let mut argv = dx_process::startup_argv(startup_options);
    argv.reserve(2 + targets.len());
    argv.push("build".to_owned());
    argv.push(profile.config_flag());
    argv.extend(targets.iter().cloned());
    let summary = format!("Running run build for {}", targets.join(" "));
    BuildPlan { argv, summary }
}

pub fn shell_join(words: &[String]) -> String {
    words
        .iter()
        .map(|word| {
            if word.is_empty()
                || word.chars().any(|cell| {
                    cell.is_whitespace() || matches!(cell, '"' | '\'' | '\\' | '$' | '`' | '!')
                })
            {
                format!("'{}'", word.replace('\'', "'\\''"))
            } else {
                word.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn plan_deploy_build(
    label: &str,
    profile: crate::args::Profile,
    startup_options: &[String],
) -> BuildPlan {
    let mut argv = dx_process::startup_argv(startup_options);
    argv.reserve(3);
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
    startup_options: &[String],
) -> BuildPlan {
    let plan = plan_run(label, app_args, profile, startup_options);
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
        let single = plan_run("//app:bin", &strings(&["--port=8080"]), Profile::Dev, &[]);
        let multi = plan_run_targets(
            &strings(&["//app:bin"]),
            &strings(&["--port=8080"]),
            Profile::Dev,
            &[],
        );
        assert_eq!(single, multi);
        let joined = plan_run_targets(&strings(&["//a:one", "//b:two"]), &[], Profile::Dev, &[]);
        assert_eq!(
            joined.argv.last(),
            Some(&"//b:two".to_owned()),
            "{joined:?}"
        );
        assert!(joined.summary.contains("//a:one //b:two"));
    }

    #[test]
    fn run_plan_forwards_app_args_verbatim() {
        let plan = plan_run("//app:bin", &strings(&["--port=8080"]), Profile::Dev, &[]);
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
        let bare = plan_run("//app:bin", &[], Profile::Dev, &[]);
        assert!(!bare.argv.contains(&"--".to_owned()));
    }

    #[test]
    fn run_profile_pins_config_flag() {
        for (profile, flag) in [
            (Profile::Debug, "--config=dx_debug"),
            (Profile::Dev, "--config=dx_dev"),
            (Profile::Release, "--config=dx_release"),
        ] {
            let plan = plan_run("//app:bin", &[], profile, &[]);
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            assert_eq!(
                argv[..5],
                ["bazel", "--nohome_rc", "--nosystem_rc", "run", flag,],
                "{profile:?}: {plan:?}"
            );
        }
    }

    #[test]
    fn run_build_plan_builds_without_launching() {
        let plan = plan_run_build(&strings(&["//a:one", "//b:two"]), Profile::Dev, &[]);
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--config=dx_dev",
                "//a:one",
                "//b:two",
            ])
        );
        assert!(!plan.argv.contains(&"run".to_owned()));
        assert!(!plan.argv.contains(&"--".to_owned()));
        assert!(plan.summary.contains("//a:one //b:two"));
    }

    #[test]
    fn run_plan_inserts_startup_options_before_the_verb() {
        let plan = plan_run(
            "//app:bin",
            &[],
            Profile::Dev,
            &["--output_base=/tmp/a".to_owned()],
        );
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_base=/tmp/a",
                "run",
                "--config=dx_dev",
                "//app:bin",
            ])
        );
        let build = plan_run_build(
            &strings(&["//a:one"]),
            Profile::Dev,
            &["--output_user_root=/tmp/b".to_owned()],
        );
        assert_eq!(
            build.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_user_root=/tmp/b",
                "build",
                "--config=dx_dev",
                "//a:one",
            ])
        );
    }

    #[test]
    fn shell_join_quotes_only_unsafe_words() {
        assert_eq!(
            shell_join(&strings(&["dx", "run", "--apply", "//app:bin"])),
            "dx run --apply //app:bin"
        );
        assert_eq!(
            shell_join(&strings(&[
                "dx",
                "run",
                "--apply",
                "//app:bin",
                "--",
                "--port=8080"
            ])),
            "dx run --apply //app:bin -- --port=8080"
        );
        assert_eq!(
            shell_join(&strings(&["dx", "run", "--", "two words"])),
            "dx run -- 'two words'"
        );
        assert_eq!(shell_join(&strings(&["o'clock"])), "'o'\\''clock'");
        assert_eq!(shell_join(&strings(&[""])), "''");
    }
}
