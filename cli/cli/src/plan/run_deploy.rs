use dx_process::ForwardError;

use super::BuildPlan;

pub fn plan_run_targets(
    targets: &[String],
    bazel_options: &[String],
    app_args: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let mut argv = super::bazel_argv(
        "run",
        &super::profile_options(Some(profile), bazel_options),
        &[],
        &[],
        targets,
        startup_options,
    )?;
    if !app_args.is_empty() {
        argv.push("--".to_owned());
        argv.extend(app_args.iter().cloned());
    }
    let summary = format!("Running run for {}", targets.join(" "));
    Ok(BuildPlan { argv, summary })
}

/// Builds the run targets without starting them.
pub fn plan_run_build(
    targets: &[String],
    bazel_options: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let argv = super::bazel_argv(
        "build",
        &super::profile_options(Some(profile), bazel_options),
        &[],
        &[],
        targets,
        startup_options,
    )?;
    let summary = format!("Running run build for {}", targets.join(" "));
    Ok(BuildPlan { argv, summary })
}

pub fn plan_run(
    target: &str,
    bazel_options: &[String],
    app_args: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    plan_run_targets(
        &[target.to_owned()],
        bazel_options,
        app_args,
        profile,
        startup_options,
    )
}

/// Quotes a word for a POSIX shell unless it is plainly safe.
pub fn shell_quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|cell| cell.is_ascii_alphanumeric() || "_@%+=:,./-".contains(cell));
    if safe {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// Joins words into one shell command line, quoting each as needed.
pub fn shell_join(words: &[String]) -> String {
    words
        .iter()
        .map(|word| shell_quote(word))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn plan_deploy_build(
    label: &str,
    profile: crate::args::Profile,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let plan = plan_run_build(&[label.to_owned()], &[], profile, startup_options)?;
    let summary = format!("Running deploy build for {label}");
    Ok(BuildPlan {
        argv: plan.argv,
        summary,
    })
}

pub fn plan_deploy_run(
    label: &str,
    app_args: &[String],
    profile: crate::args::Profile,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let plan = plan_run(label, &[], app_args, profile, startup_options)?;
    let summary = format!("Running deploy run for {label}");
    Ok(BuildPlan {
        argv: plan.argv,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Profile;
    use crate::plan::BAZEL_SETTINGS;
    use crate::test_support::strings;

    fn expected(verb: &str, rest: &[&str]) -> Vec<String> {
        let split = rest
            .iter()
            .position(|arg| !arg.starts_with("--"))
            .unwrap_or(rest.len());
        let mut argv = strings(&["bazel", "--nohome_rc", "--nosystem_rc", verb]);
        argv.extend(dx_process::build_verb_options(verb, &[]));
        argv.extend(strings(&rest[..split]));
        argv.extend(strings(&BAZEL_SETTINGS));
        argv.extend(strings(&rest[split..]));
        argv
    }

    #[test]
    fn run_targets_share_single_builder() {
        let single = plan_run(
            "//app:bin",
            &[],
            &strings(&["--port=8080"]),
            Profile::Dev,
            &[],
        )
        .expect("plan");
        let multi = plan_run_targets(
            &strings(&["//app:bin"]),
            &[],
            &strings(&["--port=8080"]),
            Profile::Dev,
            &[],
        )
        .expect("plan");
        assert_eq!(single, multi);
        let joined = plan_run_targets(
            &strings(&["//a:one", "//b:two"]),
            &[],
            &[],
            Profile::Dev,
            &[],
        )
        .expect("plan");
        assert_eq!(
            joined.argv.last(),
            Some(&"//b:two".to_owned()),
            "{joined:?}"
        );
        assert!(joined.summary.contains("//a:one //b:two"));
    }

    #[test]
    fn run_plan_forwards_app_args_verbatim() {
        let plan = plan_run(
            "//app:bin",
            &[],
            &strings(&["--port=8080"]),
            Profile::Dev,
            &[],
        )
        .expect("plan");
        assert_eq!(
            plan.argv,
            expected(
                "run",
                &["--config=dx_dev", "//app:bin", "--", "--port=8080"]
            )
        );
        assert!(plan.summary.contains("//app:bin"));
        let bare = plan_run("//app:bin", &[], &[], Profile::Dev, &[]).expect("plan");
        assert!(!bare.argv.contains(&"--".to_owned()));
    }

    #[test]
    fn run_profile_pins_config_flag() {
        for (profile, flag) in [
            (Profile::Debug, "--config=dx_debug"),
            (Profile::Dev, "--config=dx_dev"),
            (Profile::Release, "--config=dx_release"),
        ] {
            let plan = plan_run("//app:bin", &[], &[], profile, &[]).expect("plan");
            assert_eq!(
                plan.argv,
                expected("run", &[flag, "//app:bin"]),
                "{profile:?}: {plan:?}"
            );
        }
    }

    #[test]
    fn run_plan_inserts_startup_options_before_the_verb() {
        let plan = plan_run(
            "//app:bin",
            &[],
            &[],
            Profile::Dev,
            &["--output_base=/tmp/a".to_owned()],
        )
        .expect("plan");
        assert_eq!(
            plan.argv[..5],
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_base=/tmp/a",
                "run",
            ])
        );
        assert_eq!(plan.argv.last(), Some(&"//app:bin".to_owned()));
    }

    #[test]
    fn run_plans_put_agent_quiet_flags_before_the_targets_and_keep_app_args() {
        let quiet = strings(&["--noshow_progress", "--curses=no"]);
        let run = plan_run(
            "//app:bin",
            &quiet,
            &strings(&["--app-flag"]),
            Profile::Dev,
            &[],
        )
        .expect("plan");
        assert_eq!(
            run.argv,
            expected(
                "run",
                &[
                    "--config=dx_dev",
                    "--noshow_progress",
                    "--curses=no",
                    "//app:bin",
                    "--",
                    "--app-flag",
                ]
            )
        );
        let build =
            plan_run_build(&strings(&["//app:bin"]), &quiet, Profile::Dev, &[]).expect("plan");
        assert_eq!(
            build.argv,
            expected(
                "build",
                &[
                    "--config=dx_dev",
                    "--noshow_progress",
                    "--curses=no",
                    "//app:bin",
                ]
            )
        );
    }

    #[test]
    fn deploy_plans_build_then_run_with_the_shared_settings() {
        let build = plan_deploy_build("//deploy:prod", Profile::Release, &[]).expect("plan");
        assert_eq!(
            build.argv,
            expected("build", &["--config=dx_release", "//deploy:prod"])
        );
        assert_eq!(build.summary, "Running deploy build for //deploy:prod");
        let run = plan_deploy_run(
            "//deploy:prod",
            &strings(&["--port=8080"]),
            Profile::Release,
            &[],
        )
        .expect("plan");
        assert_eq!(
            run.argv,
            expected(
                "run",
                &["--config=dx_release", "//deploy:prod", "--", "--port=8080"]
            )
        );
        assert_eq!(run.summary, "Running deploy run for //deploy:prod");
    }

    #[test]
    fn run_plans_refuse_a_conflicting_shared_setting() {
        let user = strings(&["--@rules_rust//rust/settings:rustc_output_diagnostics=false"]);
        let run = plan_run("//app:bin", &user, &[], Profile::Dev, &[]);
        assert!(
            matches!(run, Err(ForwardError::ConflictingOption { .. })),
            "{run:?}"
        );
        let build = plan_run_build(&strings(&["//app:bin"]), &user, Profile::Dev, &[]);
        assert!(
            matches!(build, Err(ForwardError::ConflictingOption { .. })),
            "{build:?}"
        );
    }

    #[test]
    fn shell_join_quotes_only_unsafe_words() {
        assert_eq!(
            shell_join(&strings(&["dx", "run", "--debug", "//app:bin"])),
            "dx run --debug //app:bin"
        );
        assert_eq!(
            shell_join(&strings(&[
                "dx",
                "run",
                "--debug",
                "//app:bin",
                "--",
                "--port=8080"
            ])),
            "dx run --debug //app:bin -- --port=8080"
        );
        assert_eq!(
            shell_join(&strings(&["dx", "run", "--", "two words"])),
            "dx run -- 'two words'"
        );
        assert_eq!(shell_join(&strings(&["o'clock"])), "'o'\\''clock'");
        assert_eq!(shell_join(&strings(&[""])), "''");
        assert_eq!(shell_join(&strings(&["a;b"])), "'a;b'");
        assert_eq!(shell_join(&strings(&["x|y"])), "'x|y'");
        assert_eq!(shell_join(&strings(&["a&b"])), "'a&b'");
        assert_eq!(shell_join(&strings(&["--out=>f"])), "'--out=>f'");
        assert_eq!(shell_join(&strings(&["<in"])), "'<in'");
        assert_eq!(shell_join(&strings(&["#tag"])), "'#tag'");
        assert_eq!(shell_join(&strings(&["*.json"])), "'*.json'");
        assert_eq!(shell_join(&strings(&["?"])), "'?'");
        assert_eq!(shell_join(&strings(&["~"])), "'~'");
        assert_eq!(shell_join(&strings(&["(x)"])), "'(x)'");
        assert_eq!(shell_join(&strings(&["{a,b}"])), "'{a,b}'");
        assert_eq!(shell_join(&strings(&["[x]"])), "'[x]'");
        assert_eq!(shell_join(&strings(&["a\nb"])), "'a\nb'");
        assert_eq!(shell_join(&strings(&["a\tb"])), "'a\tb'");
        assert_eq!(
            shell_join(&strings(&["--name=a;rm -rf x"])),
            "'--name=a;rm -rf x'"
        );
        assert_eq!(shell_join(&strings(&["é"])), "'é'");
        assert_eq!(shell_join(&strings(&["^x"])), "'^x'");
        assert_eq!(
            shell_join(&strings(&[
                "--@rules_dx//config:clippy_output_diagnostics=true"
            ])),
            "--@rules_dx//config:clippy_output_diagnostics=true"
        );
        assert_eq!(shell_join(&strings(&["--config=ci"])), "--config=ci");
        assert_eq!(shell_join(&strings(&["--copt=-O2"])), "--copt=-O2");
        assert_eq!(shell_join(&strings(&["@repo//pkg:t"])), "@repo//pkg:t");
        assert_eq!(shell_join(&strings(&["100%"])), "100%");
        assert_eq!(shell_join(&strings(&["a,b+c_d.e"])), "a,b+c_d.e");
        assert_eq!(
            shell_join(&strings(&[
                "dx",
                "deploy",
                "--apply",
                "--",
                "--define=x=a;b",
                "--copt=a b"
            ])),
            "dx deploy --apply -- '--define=x=a;b' '--copt=a b'"
        );
    }
}
