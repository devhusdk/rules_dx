use dx_process::{build_workflow_argv, describe_scope, ForwardError, ProtectedFlag};

use super::{workflow_scope_labels, BuildPlan, BEP_FLAG_NAME, DOWNLOAD_ALL_FLAG};
use crate::resolve::ResolvedScope;

pub use crate::args::WorkflowVerb;

pub const COVERAGE_COMBINED_REPORT_FLAG: &str = "--combined_report=lcov";

fn is_dx_profile_config(value: &str) -> bool {
    [
        crate::args::Profile::Debug,
        crate::args::Profile::Dev,
        crate::args::Profile::Release,
    ]
    .iter()
    .any(|profile| profile.config() == value)
}

fn filter_configs(
    options: &[String],
    profile: Option<crate::args::Profile>,
) -> Result<Vec<String>, ForwardError> {
    let Some(profile) = profile else {
        return Ok(options.to_vec());
    };
    let required = profile.config_flag();
    let mut kept = Vec::with_capacity(options.len());
    let mut index = 0;
    while index < options.len() {
        let arg = &options[index];
        if arg == &required {
            index += 1;
            continue;
        }
        let (value, consumed) = if let Some(value) = arg.strip_prefix("--config=") {
            (Some(value.to_owned()), 1)
        } else if arg == "--config" {
            match options.get(index + 1) {
                Some(value) => (Some(value.clone()), 2),
                None => (None, 1),
            }
        } else {
            kept.push(arg.clone());
            index += 1;
            continue;
        };
        if consumed == 2 && Some(profile.config()) == value.as_deref() {
            index += consumed;
            continue;
        }
        if let Some(value) = value {
            if is_dx_profile_config(&value) {
                return Err(ForwardError::ConflictingOption {
                    flag: "config".to_owned(),
                });
            }
        }
        for offset in 0..consumed {
            kept.push(options[index + offset].clone());
        }
        index += consumed;
    }
    Ok(kept)
}

pub fn workflow_options(
    verb: WorkflowVerb,
    bep_path: Option<&str>,
    profile: Option<crate::args::Profile>,
) -> Vec<String> {
    let mut required = Vec::new();
    if let Some(profile) = profile {
        required.push(profile.config_flag());
    }
    if verb == WorkflowVerb::Coverage {
        required.push(COVERAGE_COMBINED_REPORT_FLAG.to_owned());
    }
    if verb.collects_reports() {
        required.push(DOWNLOAD_ALL_FLAG.to_owned());
    }
    if let Some(path) = bep_path {
        required.push(format!("--{BEP_FLAG_NAME}={path}"));
    }
    required
}

pub fn workflow_protected(verb: WorkflowVerb) -> Vec<ProtectedFlag> {
    let mut protected = Vec::new();
    if verb == WorkflowVerb::Coverage {
        protected.push(ProtectedFlag {
            name: "combined_report".to_owned(),
            required: Some(COVERAGE_COMBINED_REPORT_FLAG.to_owned()),
            allowed: Vec::new(),
        });
    }
    if verb.collects_reports() {
        protected.push(ProtectedFlag {
            name: "remote_download_outputs".to_owned(),
            required: Some(DOWNLOAD_ALL_FLAG.to_owned()),
            allowed: Vec::new(),
        });
    }
    protected.push(ProtectedFlag {
        name: BEP_FLAG_NAME.to_owned(),
        required: None,
        allowed: Vec::new(),
    });
    protected
}

pub fn plan_workflow(
    verb: WorkflowVerb,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    bep_path: Option<&str>,
    profile: Option<crate::args::Profile>,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let required = workflow_options(verb, bep_path, profile);
    let protected = workflow_protected(verb);
    let bazel_options = filter_configs(bazel_options, profile)?;
    let (scope, labels) = workflow_scope_labels(resolved);
    let argv = build_workflow_argv(
        verb.name(),
        &bazel_options,
        &required,
        &protected,
        &labels,
        startup_options,
    )?;
    let summary = format!("Running {} for {}", verb.name(), describe_scope(&scope));
    Ok(BuildPlan { argv, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Profile;
    use crate::test_support::strings;
    use dx_process::Scope;

    fn resolved(targets: &[&str]) -> ResolvedScope {
        ResolvedScope {
            scope: if targets.is_empty() {
                Scope::Repository
            } else {
                Scope::Labels(strings(targets))
            },
            targets: strings(targets),
        }
    }

    #[test]
    fn workflow_plan_defaults_to_fail_fast_without_forced_keep_going() {
        for (verb, profile) in [
            (WorkflowVerb::Build, Some(Profile::Dev)),
            (WorkflowVerb::Test, Some(Profile::Dev)),
            (WorkflowVerb::Coverage, None),
        ] {
            let plan = plan_workflow(verb, &resolved(&[]), &[], None, profile, &[]).expect("plan");
            assert!(
                !plan.argv.iter().any(|arg| arg == "--keep_going"),
                "{verb:?} must not force keep_going: {plan:?}"
            );
        }
        let plan = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &[],
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(
            argv[..5],
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "test",
                "--config=dx_dev",
            ]
        );
        assert_eq!(argv[5..], ["--remote_download_outputs=all", "//..."]);
    }

    #[test]
    fn workflow_plan_forwards_explicit_keep_going() {
        let plan = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--keep_going"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("plan");
        assert!(plan.argv.iter().any(|arg| arg == "--keep_going"));
    }

    #[test]
    fn workflow_plan_forwards_test_binary_args_on_test_verbs() {
        for verb in [WorkflowVerb::Test, WorkflowVerb::Coverage] {
            let plan = plan_workflow(
                verb,
                &resolved(&[]),
                &strings(&[
                    "--test_arg=--exact",
                    "--test_arg",
                    "case with spaces héllo",
                    "--test_filter=unit",
                ]),
                None,
                if verb == WorkflowVerb::Coverage {
                    None
                } else {
                    Some(Profile::Dev)
                },
                &[],
            )
            .expect("test args forward");
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            let position = argv
                .iter()
                .position(|arg| *arg == "--test_arg=--exact")
                .expect("test_arg is planned");
            assert_eq!(
                &argv[position..position + 4],
                &[
                    "--test_arg=--exact",
                    "--test_arg",
                    "case with spaces héllo",
                    "--test_filter=unit",
                ],
                "{verb:?}: {plan:?}"
            );
        }
        let err = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--test_arg=--exact"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect_err("build still rejects test args");
        assert!(
            matches!(err, ForwardError::TestBinaryArgs { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn workflow_plan_forwards_consumer_configs_after_profile() {
        for verb in [WorkflowVerb::Build, WorkflowVerb::Test] {
            let plan = plan_workflow(
                verb,
                &resolved(&[]),
                &strings(&[
                    "--keep_going",
                    "--config=ci",
                    "--config=ci-pr",
                    "--config=sanitizer",
                ]),
                None,
                Some(Profile::Dev),
                &[],
            )
            .expect("consumer configs pass");
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            let dev = argv
                .iter()
                .position(|arg| *arg == "--config=dx_dev")
                .expect("profile is planned");
            let keep_going = argv
                .iter()
                .position(|arg| *arg == "--keep_going")
                .expect("keep_going is planned");
            let ci = argv
                .iter()
                .position(|arg| *arg == "--config=ci")
                .expect("ci is planned");
            let ci_pr = argv
                .iter()
                .position(|arg| *arg == "--config=ci-pr")
                .expect("ci-pr is planned");
            let sanitizer = argv
                .iter()
                .position(|arg| *arg == "--config=sanitizer")
                .expect("sanitizer is planned");
            assert!(dev < keep_going, "{verb:?}: {plan:?}");
            assert!(
                keep_going < ci && ci < ci_pr && ci_pr < sanitizer,
                "{verb:?}: {plan:?}"
            );
        }
    }

    #[test]
    fn workflow_plan_rejects_only_conflicting_dx_profile() {
        for options in [
            vec!["--config=dx_release"],
            vec!["--config", "dx_release"],
            vec!["--config=sanitizer", "--config=dx_debug"],
        ] {
            let err = plan_workflow(
                WorkflowVerb::Build,
                &resolved(&[]),
                &strings(&options),
                None,
                Some(Profile::Dev),
                &[],
            )
            .expect_err("conflicting dx profile must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "got {err:?}"
            );
        }
        let err = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--config=dx_release"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect_err("test keeps the profile conflict");
        assert!(matches!(err, ForwardError::ConflictingOption { .. }));
    }

    #[test]
    fn workflow_plan_dedupes_repeated_profile_config() {
        let plan = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--config=dx_dev", "--config=sanitizer"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("repeated profile config is accepted");
        assert_eq!(
            plan.argv
                .iter()
                .filter(|arg| *arg == "--config=dx_dev")
                .count(),
            1,
            "{plan:?}"
        );
        let plan = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--config", "dx_dev", "--config", "sanitizer"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("bare repeated profile config is accepted");
        assert_eq!(
            plan.argv
                .iter()
                .filter(|arg| *arg == "--config=dx_dev")
                .count(),
            1,
            "{plan:?}"
        );
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        let bare = argv.iter().position(|arg| *arg == "--config");
        assert_eq!(
            argv[bare.expect("bare consumer config is planned") + 1],
            "sanitizer",
            "{plan:?}"
        );
    }

    #[test]
    fn workflow_plan_forwards_any_config_without_profile() {
        for options in [vec!["--config=sanitizer"], vec!["--config=dx_release"]] {
            let plan = plan_workflow(
                WorkflowVerb::Coverage,
                &resolved(&[]),
                &strings(&options),
                None,
                None,
                &[],
            )
            .expect("coverage has no profile pin to conflict with");
            assert!(plan.argv.iter().any(|arg| arg == &options[0]), "{plan:?}");
        }
    }

    #[test]
    fn coverage_plan_requires_combined_lcov_report() {
        let plan = plan_workflow(WorkflowVerb::Coverage, &resolved(&[]), &[], None, None, &[])
            .expect("plan");
        assert!(plan
            .argv
            .iter()
            .any(|arg| arg == COVERAGE_COMBINED_REPORT_FLAG));
        let repeated = plan_workflow(
            WorkflowVerb::Coverage,
            &resolved(&[]),
            &strings(&[COVERAGE_COMBINED_REPORT_FLAG]),
            None,
            None,
            &[],
        )
        .expect("repeated required flag is accepted");
        assert!(repeated
            .argv
            .iter()
            .any(|arg| arg == COVERAGE_COMBINED_REPORT_FLAG));
        let err = plan_workflow(
            WorkflowVerb::Coverage,
            &resolved(&[]),
            &strings(&["--combined_report=json"]),
            None,
            None,
            &[],
        )
        .expect_err("conflicting combined_report must fail");
        assert!(
            matches!(err, ForwardError::ConflictingOption { .. }),
            "got {err:?}"
        );
        let err = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--build_event_json_file=/tmp/other.json"]),
            Some("/tmp/bep.json"),
            Some(Profile::Dev),
            &[],
        )
        .expect_err("BEP override must fail");
        assert!(
            matches!(err, ForwardError::ConflictingOption { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn workflow_profile_pins_config_flag_in_order() {
        for (profile, flag) in [
            (Profile::Debug, "--config=dx_debug"),
            (Profile::Dev, "--config=dx_dev"),
            (Profile::Release, "--config=dx_release"),
        ] {
            let plan = plan_workflow(
                WorkflowVerb::Build,
                &resolved(&[]),
                &[],
                None,
                Some(profile),
                &[],
            )
            .expect("plan");
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            assert_eq!(
                argv[..5],
                ["bazel", "--nohome_rc", "--nosystem_rc", "build", flag,],
                "{profile:?}: {plan:?}"
            );
        }
        let plan = plan_workflow(WorkflowVerb::Coverage, &resolved(&[]), &[], None, None, &[])
            .expect("plan");
        assert!(
            !plan.argv.iter().any(|arg| arg.starts_with("--config=")),
            "coverage argv is unchanged: {plan:?}"
        );
        let repeated = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--config=dx_dev"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("repeated required config is accepted");
        assert!(repeated.argv.contains(&"--config=dx_dev".to_owned()));
        let err = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--config=dx_release"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect_err("conflicting config must fail");
        assert!(
            matches!(err, ForwardError::ConflictingOption { .. }),
            "got {err:?}"
        );
    }
}
