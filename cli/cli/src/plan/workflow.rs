use dx_process::{build_workflow_argv, describe_scope, ForwardError, ProtectedFlag};

use super::{workflow_scope_labels, BuildPlan, BEP_FLAG_NAME, DOWNLOAD_ALL_FLAG};
use crate::resolve::ResolvedScope;

pub use crate::args::WorkflowVerb;

pub const COVERAGE_COMBINED_REPORT_FLAG: &str = "--combined_report=lcov";

pub const BLESSED_EXTRA_CONFIGS: [&str; 2] = ["--config=ci", "--config=ci-pr"];

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

pub fn workflow_protected(
    verb: WorkflowVerb,
    profile: Option<crate::args::Profile>,
) -> Vec<ProtectedFlag> {
    let mut protected = Vec::new();
    if let Some(profile) = profile {
        protected.push(ProtectedFlag {
            name: "config".to_owned(),
            required: Some(profile.config_flag()),
            allowed: BLESSED_EXTRA_CONFIGS
                .iter()
                .map(ToString::to_string)
                .collect(),
        });
    }
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
) -> Result<BuildPlan, ForwardError> {
    let required = workflow_options(verb, bep_path, profile);
    let protected = workflow_protected(verb, profile);
    let (scope, labels) = workflow_scope_labels(resolved);
    let argv = build_workflow_argv(verb.name(), bazel_options, &required, &protected, &labels)?;
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
            let plan = plan_workflow(verb, &resolved(&[]), &[], None, profile).expect("plan");
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
        )
        .expect("plan");
        assert!(plan.argv.iter().any(|arg| arg == "--keep_going"));
    }

    #[test]
    fn workflow_plan_accepts_blessed_ci_configs_beside_profile() {
        for verb in [WorkflowVerb::Build, WorkflowVerb::Test] {
            let plan = plan_workflow(
                verb,
                &resolved(&[]),
                &strings(&["--config=ci", "--config=ci-pr"]),
                None,
                Some(Profile::Dev),
            )
            .expect("blessed configs pass");
            assert!(plan.argv.iter().any(|arg| arg == "--config=dx_dev"));
            assert!(plan.argv.iter().any(|arg| arg == "--config=ci"));
            assert!(plan.argv.iter().any(|arg| arg == "--config=ci-pr"));
        }
        let err = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--config=dx_release"]),
            None,
            Some(Profile::Dev),
        )
        .expect_err("profile override still conflicts");
        assert!(matches!(err, ForwardError::ConflictingOption { .. }));
    }

    #[test]
    fn coverage_plan_requires_combined_lcov_report() {
        let plan =
            plan_workflow(WorkflowVerb::Coverage, &resolved(&[]), &[], None, None).expect("plan");
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
            )
            .expect("plan");
            let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
            assert_eq!(
                argv[..5],
                ["bazel", "--nohome_rc", "--nosystem_rc", "build", flag,],
                "{profile:?}: {plan:?}"
            );
        }
        let plan =
            plan_workflow(WorkflowVerb::Coverage, &resolved(&[]), &[], None, None).expect("plan");
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
        )
        .expect("repeated required config is accepted");
        assert!(repeated.argv.contains(&"--config=dx_dev".to_owned()));
        let err = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--config=dx_release"]),
            None,
            Some(Profile::Dev),
        )
        .expect_err("conflicting config must fail");
        assert!(
            matches!(err, ForwardError::ConflictingOption { .. }),
            "got {err:?}"
        );
    }
}
