use dx_process::{describe_scope, ForwardError, ProtectedFlag};

use super::{
    bazel_argv, bep_guards, bep_options, download_flag, download_guards, profile_options,
    workflow_scope_labels, BuildPlan, SKIP_INCOMPATIBLE_FLAG,
};
use crate::resolve::ResolvedScope;

pub use crate::args::WorkflowVerb;

pub const COVERAGE_COMBINED_REPORT_FLAG: &str = "--combined_report=lcov";

pub const COVERAGE_SOURCES_GROUP_FLAG: &str = "--output_groups=+dx_coverage_sources";
pub const COVERAGE_ALL_TARGETS_FLAG: &str = "--build_tests_only=false";

/// The `--aspects` flag that writes the coverage source manifests.
pub fn coverage_sources_aspect_flag() -> String {
    format!(
        "--aspects={}",
        super::rules_dx_label(dx_lcov::SOURCES_ASPECT)
    )
}

fn workflow_defaults(verb: WorkflowVerb) -> &'static [&'static str] {
    match verb {
        WorkflowVerb::Build | WorkflowVerb::Test | WorkflowVerb::Coverage => {
            &[SKIP_INCOMPATIBLE_FLAG]
        }
        WorkflowVerb::Run => &[],
    }
}

pub fn workflow_options(
    verb: WorkflowVerb,
    bazel_options: &[String],
    bep_path: Option<&str>,
) -> Vec<String> {
    let mut required = Vec::new();
    if verb == WorkflowVerb::Coverage {
        required.push(COVERAGE_COMBINED_REPORT_FLAG.to_owned());
        required.push(COVERAGE_ALL_TARGETS_FLAG.to_owned());
        required.push(coverage_sources_aspect_flag());
        required.push(COVERAGE_SOURCES_GROUP_FLAG.to_owned());
    }
    if verb.collects_reports() {
        required.push(download_flag(bazel_options).to_owned());
    }
    if let Some(path) = bep_path {
        required.extend(bep_options(path));
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
        protected.push(ProtectedFlag {
            name: "build_tests_only".to_owned(),
            required: None,
            allowed: vec![
                "--build_tests_only=false".to_owned(),
                "--build_tests_only=0".to_owned(),
                "--build_tests_only=no".to_owned(),
            ],
        });
    }
    if verb.collects_reports() {
        protected.extend(download_guards());
    }
    protected.extend(bep_guards());
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
    let required = workflow_options(verb, bazel_options, bep_path);
    let protected = workflow_protected(verb);
    let mut options: Vec<String> = workflow_defaults(verb)
        .iter()
        .map(ToString::to_string)
        .collect();
    options.extend(crate::args::filter_profile_configs(bazel_options, profile)?);
    let bazel_options = profile_options(profile, &options);
    let (scope, labels) = workflow_scope_labels(resolved);
    let argv = bazel_argv(
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
    use dx_process::Scope;
    use dx_testing::cli::strings;

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
            argv,
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "test",
                "--config=dx_dev",
                SKIP_INCOMPATIBLE_FLAG,
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
                "--remote_download_outputs=toplevel",
                "//...",
            ]
        );
    }

    #[test]
    fn workflow_plan_skips_incompatible_targets_except_for_run() {
        for verb in [
            WorkflowVerb::Build,
            WorkflowVerb::Test,
            WorkflowVerb::Coverage,
        ] {
            let plan =
                plan_workflow(verb, &resolved(&["//a:x"]), &[], None, None, &[]).expect("plan");
            assert!(
                plan.argv
                    .iter()
                    .any(|arg| arg == "--skip_incompatible_explicit_targets"),
                "{verb:?} skips incompatible targets: {plan:?}"
            );
        }
        let plan = plan_workflow(
            WorkflowVerb::Run,
            &resolved(&["//a:x"]),
            &[],
            None,
            None,
            &[],
        )
        .expect("plan");
        assert!(
            !plan
                .argv
                .iter()
                .any(|arg| arg.contains("skip_incompatible_explicit_targets")),
            "run keeps strict compatibility: {plan:?}"
        );
    }

    #[test]
    fn workflow_plan_guards_the_shared_settings() {
        for conflicting in [
            "--@rules_rust//rust/settings:rustc_output_diagnostics=false",
            "--@rules_dx//config:clippy_output_diagnostics=false",
        ] {
            let err = plan_workflow(
                WorkflowVerb::Build,
                &resolved(&[]),
                &strings(&[conflicting]),
                None,
                Some(Profile::Dev),
                &[],
            )
            .expect_err("conflicting setting must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
        let same = "--@rules_rust//rust/settings:rustc_output_diagnostics=true";
        let plan = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&[same, crate::plan::CLIPPY_DIAGNOSTICS_FLAG]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("matching values pass");
        assert!(plan.argv.iter().any(|arg| arg == same), "{plan:?}");
    }

    #[test]
    fn workflow_plan_keeps_build_event_paths_local() {
        for conflicting in [
            "--build_event_json_file_path_conversion",
            "--build_event_json_file_path_conversion=true",
        ] {
            let err = plan_workflow(
                WorkflowVerb::Build,
                &resolved(&[]),
                &strings(&[conflicting]),
                Some("/tmp/bep.json"),
                Some(Profile::Dev),
                &[],
            )
            .expect_err("path conversion must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
        let plan = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &strings(&["--nobuild_event_json_file_path_conversion"]),
            Some("/tmp/bep.json"),
            Some(Profile::Dev),
            &[],
        )
        .expect("local paths pass");
        assert!(plan
            .argv
            .iter()
            .any(|arg| arg == "--build_event_json_file_path_conversion=false"));
        let plan = plan_workflow(
            WorkflowVerb::Build,
            &resolved(&[]),
            &[],
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("plan");
        assert!(
            !plan
                .argv
                .iter()
                .any(|arg| arg.starts_with("--build_event_json_file")),
            "{plan:?}"
        );
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
                    "--config=mine",
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
                .position(|arg| *arg == "--config=mine")
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
    fn coverage_plan_requests_the_source_manifests() {
        let aspect = coverage_sources_aspect_flag();
        assert_eq!(
            aspect,
            "--aspects=@rules_dx//quality:coverage_sources.bzl%dx_coverage_sources_aspect"
        );
        let user = strings(&[
            "--aspects=//tools:mine.bzl%mine",
            "--output_groups=default",
            "--nobuild_tests_only",
            "--build_tests_only=false",
        ]);
        let plan = plan_workflow(
            WorkflowVerb::Coverage,
            &resolved(&[]),
            &user,
            None,
            None,
            &[],
        )
        .expect("plan");
        let position = |flag: &str| {
            plan.argv
                .iter()
                .position(|arg| arg == flag)
                .unwrap_or_else(|| panic!("{flag} is planned: {plan:?}"))
        };
        assert!(position(&aspect) > position("--aspects=//tools:mine.bzl%mine"));
        assert!(position(COVERAGE_SOURCES_GROUP_FLAG) > position("--output_groups=default"));
        let forced_last = plan
            .argv
            .iter()
            .rposition(|arg| arg == COVERAGE_ALL_TARGETS_FLAG)
            .expect("build_tests_only forced off");
        assert!(forced_last > position("--build_tests_only=false"));
        let bare = plan_workflow(
            WorkflowVerb::Coverage,
            &resolved(&[]),
            &strings(&["--config=mine"]),
            None,
            None,
            &[],
        )
        .expect("plan");
        let forced = bare
            .argv
            .iter()
            .position(|arg| arg == COVERAGE_ALL_TARGETS_FLAG)
            .unwrap_or_else(|| panic!("rc build_tests_only is overridden: {bare:?}"));
        let config = bare
            .argv
            .iter()
            .position(|arg| arg == "--config=mine")
            .expect("user config kept");
        assert!(forced > config, "forced after rc expansion: {bare:?}");
        for verb in [WorkflowVerb::Build, WorkflowVerb::Test] {
            let plan = plan_workflow(verb, &resolved(&[]), &[], None, None, &[]).expect("plan");
            assert!(
                !plan
                    .argv
                    .iter()
                    .any(|arg| arg == &aspect || arg == COVERAGE_SOURCES_GROUP_FLAG),
                "{verb:?} writes no source manifests: {plan:?}"
            );
        }
    }

    #[test]
    fn coverage_plan_refuses_build_tests_only() {
        for flag in ["--build_tests_only", "--build_tests_only=true"] {
            let err = plan_workflow(
                WorkflowVerb::Coverage,
                &resolved(&[]),
                &strings(&[flag]),
                None,
                None,
                &[],
            )
            .expect_err("build_tests_only hides sources");
            assert!(
                matches!(err, ForwardError::ConflictingOption { ref flag } if flag == "build_tests_only"),
                "{flag}: {err:?}"
            );
        }
        plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--build_tests_only"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("test keeps build_tests_only");
    }

    #[test]
    fn workflow_required_flags_follow_user_config() {
        let plan = plan_workflow(
            WorkflowVerb::Coverage,
            &resolved(&[]),
            &strings(&["--config=remote", "--jobs=4"]),
            Some("/tmp/bep.json"),
            None,
            &[],
        )
        .expect("plan");
        let position = |flag: &str| {
            plan.argv
                .iter()
                .position(|arg| arg == flag)
                .unwrap_or_else(|| panic!("{flag} is planned: {plan:?}"))
        };
        let remote = position("--config=remote");
        for flag in [
            COVERAGE_COMBINED_REPORT_FLAG,
            crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            "--build_event_json_file=/tmp/bep.json",
            "--build_event_json_file_path_conversion=false",
        ] {
            assert!(position(flag) > remote, "{flag}: {plan:?}");
        }
        assert!(position("--jobs=4") < position(crate::plan::DOWNLOAD_TOPLEVEL_FLAG));
    }

    #[test]
    fn workflow_profile_config_leads_user_options() {
        let plan = plan_workflow(
            WorkflowVerb::Test,
            &resolved(&[]),
            &strings(&["--config=remote", "--jobs=4"]),
            None,
            Some(Profile::Dev),
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(
            argv,
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "test",
                "--config=dx_dev",
                SKIP_INCOMPATIBLE_FLAG,
                "--config=remote",
                "--jobs=4",
                crate::plan::CLIPPY_DIAGNOSTICS_FLAG,
                crate::plan::RUSTC_DIAGNOSTICS_FLAG,
                crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
                "//...",
            ]
        );
    }

    #[test]
    fn report_verbs_reject_download_expansion_flags() {
        for verb in [WorkflowVerb::Test, WorkflowVerb::Coverage] {
            for (option, flag) in [
                ("--remote_download_minimal", "remote_download_minimal"),
                (
                    "--remote_download_outputs=minimal",
                    "remote_download_outputs",
                ),
            ] {
                let err = plan_workflow(verb, &resolved(&[]), &strings(&[option]), None, None, &[])
                    .expect_err("download override must fail");
                assert_eq!(
                    err,
                    ForwardError::ConflictingOption {
                        flag: flag.to_owned()
                    },
                    "{verb:?} {option}"
                );
            }
        }
        for option in ["--remote_download_minimal", "--remote_download_toplevel"] {
            let plan = plan_workflow(
                WorkflowVerb::Run,
                &resolved(&[]),
                &strings(&[option]),
                None,
                Some(Profile::Dev),
                &[],
            )
            .expect("run forces no download mode");
            assert!(plan.argv.iter().any(|arg| arg == option), "{plan:?}");
        }
    }

    #[test]
    fn report_verbs_follow_last_download_choice() {
        let last_download = |argv: &[String]| {
            argv.iter()
                .rev()
                .find(|arg| arg.starts_with("--remote_download"))
                .cloned()
        };
        for verb in [WorkflowVerb::Test, WorkflowVerb::Coverage] {
            for (options, expected) in [
                (
                    vec!["--remote_download_toplevel"],
                    crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
                ),
                (
                    vec!["--remote_download_outputs=toplevel"],
                    crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
                ),
                (
                    vec!["--remote_download_all"],
                    crate::plan::DOWNLOAD_ALL_FLAG,
                ),
                (
                    vec!["--remote_download_outputs=all"],
                    crate::plan::DOWNLOAD_ALL_FLAG,
                ),
                (
                    vec!["--remote_download_all", "--remote_download_toplevel"],
                    crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
                ),
            ] {
                let plan = plan_workflow(verb, &resolved(&[]), &strings(&options), None, None, &[])
                    .expect("plan");
                assert_eq!(
                    last_download(&plan.argv).as_deref(),
                    Some(expected),
                    "{verb:?} {options:?}"
                );
            }
        }
        for verb in [WorkflowVerb::Build, WorkflowVerb::Run] {
            let plan = plan_workflow(verb, &resolved(&[]), &[], None, None, &[]).expect("plan");
            assert!(
                !plan
                    .argv
                    .iter()
                    .any(|arg| arg.starts_with("--remote_download_outputs=")),
                "{verb:?}: {plan:?}"
            );
        }
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
                argv[..8],
                [
                    "bazel",
                    "--nohome_rc",
                    "--nosystem_rc",
                    "build",
                    flag,
                    SKIP_INCOMPATIBLE_FLAG,
                    crate::plan::CLIPPY_DIAGNOSTICS_FLAG,
                    crate::plan::RUSTC_DIAGNOSTICS_FLAG,
                ],
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
