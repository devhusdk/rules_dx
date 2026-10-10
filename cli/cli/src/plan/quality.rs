use dx_process::{
    build_workflow_argv, describe_scope, operation_summary, ForwardError, ProtectedFlag, Scope,
};

use super::{
    registry::{spec, CommandSpec},
    BuildPlan, BEP_FLAG_NAME, DOWNLOAD_ALL_FLAG, KEEP_GOING_FLAG, OUTPUT_GROUP, VALIDATE_FLAG,
};
use crate::args::Command;
use crate::resolve::ResolvedScope;

pub fn required_options(entry: &CommandSpec, bep_path: &str) -> Vec<String> {
    let mut options = vec![
        format!("--aspects={}", entry.aspects.join(",")),
        format!("--output_groups={OUTPUT_GROUP}"),
        DOWNLOAD_ALL_FLAG.to_owned(),
        VALIDATE_FLAG.to_owned(),
        KEEP_GOING_FLAG.to_owned(),
        format!("--{BEP_FLAG_NAME}={bep_path}"),
    ];
    options.extend(entry.settings.iter().map(ToString::to_string));
    options
}

fn setting_name(option: &str) -> Result<String, ForwardError> {
    let bare = option
        .strip_prefix("--")
        .ok_or_else(|| ForwardError::InvalidSetting {
            option: option.to_owned(),
        })?;
    let name = bare.split('=').next().unwrap_or("");
    if name.is_empty() {
        return Err(ForwardError::InvalidSetting {
            option: option.to_owned(),
        });
    }
    Ok(name.to_owned())
}

pub fn protected_flags(
    required: &[String],
    settings: &[&str],
) -> Result<Vec<ProtectedFlag>, ForwardError> {
    let keep_going = required
        .iter()
        .find(|option| option.as_str() == KEEP_GOING_FLAG)
        .cloned()
        .ok_or_else(|| ForwardError::InvalidRequiredOption {
            flag: "keep_going".to_owned(),
        })?;
    let mut flags = vec![
        ProtectedFlag {
            name: "aspects".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "output_groups".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "remote_download_outputs".to_owned(),
            required: Some(DOWNLOAD_ALL_FLAG.to_owned()),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "@rules_dx//config:validate".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "keep_going".to_owned(),
            required: Some(keep_going),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "nokeep_going".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: BEP_FLAG_NAME.to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ];
    for setting in settings {
        flags.push(ProtectedFlag {
            name: setting_name(setting)?,
            required: None,
            allowed: Vec::new(),
        });
    }
    Ok(flags)
}

pub(crate) fn workflow_scope_labels(resolved: &ResolvedScope) -> (Scope, Vec<String>) {
    if resolved.targets.is_empty() {
        (Scope::Repository, vec![describe_scope(&Scope::Repository)])
    } else {
        (resolved.scope.clone(), resolved.targets.clone())
    }
}

pub fn plan_build(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let entry = spec(command);
    let required = required_options(&entry, bep_path);
    let protected = protected_flags(&required, entry.settings)?;
    let (scope, labels) = workflow_scope_labels(resolved);
    let argv = build_workflow_argv(
        "build",
        bazel_options,
        &required,
        &protected,
        &labels,
        startup_options,
    )?;
    let summary = operation_summary(command.name(), "analysis", &scope);
    Ok(BuildPlan { argv, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{parse, ArgsError};
    use crate::plan::{CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG};
    use crate::resolve::{resolve, QueryResult, QueryRunner};
    use crate::test_support::strings;
    use dx_process::Scope;
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

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
    fn protected_flags_find_keep_going_by_value() {
        let entry = spec(Command::Lint);
        let required = required_options(&entry, "/tmp/bep.json");
        let flags = protected_flags(&required, entry.settings).expect("flags");
        let keep_going = flags
            .iter()
            .find(|flag| flag.name == "keep_going")
            .expect("keep_going guard");
        assert_eq!(keep_going.required, Some(KEEP_GOING_FLAG.to_owned()));
    }

    #[test]
    fn protected_flags_reject_miswired_required_and_settings() {
        let entry = spec(Command::Lint);
        let mut required = required_options(&entry, "/tmp/bep.json");
        required.retain(|option| option.as_str() != KEEP_GOING_FLAG);
        let err = protected_flags(&required, entry.settings).expect_err("missing keep_going");
        assert!(
            matches!(err, ForwardError::InvalidRequiredOption { .. }),
            "missing keep_going produced {err:?}"
        );
        let required = required_options(&entry, "/tmp/bep.json");
        let err = protected_flags(&required, &["clippy_output_diagnostics=true"])
            .expect_err("malformed setting");
        assert!(
            matches!(err, ForwardError::InvalidSetting { .. }),
            "malformed setting produced {err:?}"
        );
    }

    #[test]
    fn lint_plan_argv_places_required_before_user_options() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--jobs=4"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(
            argv[..7],
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--aspects=//quality:real_aspects.bzl%real_lint_aspect,//quality:real_aspects.bzl%real_js_lint_aspect,//quality:real_aspects.bzl%real_python_lint_aspect,//quality:real_aspects.bzl%real_jvm_lint_aspect,//quality:real_aspects.bzl%real_rust_lint_aspect,//quality:real_aspects.bzl%real_shell_lint_aspect",
                "--output_groups=dx_results",
                "--remote_download_outputs=all",
            ]
        );
        assert_eq!(
            argv[7..],
            [
                "--@rules_dx//config:validate=false",
                "--keep_going",
                "--build_event_json_file=/tmp/bep.json",
                "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
                "--jobs=4",
                "//...",
            ]
        );
        assert_eq!(plan.summary, "Running lint analysis for //...");
    }

    #[test]
    fn lint_plan_enables_upstream_clippy_diagnostics() {
        let plan =
            plan_build(Command::Lint, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg == CLIPPY_DIAGNOSTICS_FLAG),
            "lint sets the upstream diagnostics setting: {plan:?}"
        );
        for command in [Command::Typecheck, Command::Format] {
            let plan =
                plan_build(command, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
            assert!(
                plan.argv.iter().all(|arg| arg != CLIPPY_DIAGNOSTICS_FLAG),
                "{command:?} leaves the clippy setting off: {plan:?}"
            );
        }
    }

    #[test]
    fn typecheck_plan_enables_upstream_rustc_diagnostics() {
        let plan = plan_build(
            Command::Typecheck,
            &resolved(&[]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg == RUSTC_DIAGNOSTICS_FLAG),
            "typecheck sets the upstream diagnostics setting: {plan:?}"
        );
        for command in [Command::Lint, Command::Format] {
            let plan =
                plan_build(command, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
            assert!(
                plan.argv.iter().all(|arg| arg != RUSTC_DIAGNOSTICS_FLAG),
                "{command:?} leaves the rustc setting off: {plan:?}"
            );
        }
    }

    #[test]
    fn explicit_targets_replace_repository_scope() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&["//a:one", "//b/..."]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(argv[argv.len() - 2..], ["//a:one", "//b/..."]);
        assert_eq!(plan.summary, "Running lint analysis for //a:one //b/...");
    }

    #[test]
    fn resolved_owners_render_sorted_owners_in_summary() {
        let scope = ResolvedScope {
            scope: Scope::ResolvedOwners(strings(&["//a:a", "//b:b"])),
            targets: strings(&["//a:a", "//b:b"]),
        };
        let plan = plan_build(Command::Lint, &scope, &[], "/tmp/bep.json", &[]).expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(argv[argv.len() - 2..], ["//a:a", "//b:b"]);
        assert_eq!(plan.summary, "Running lint analysis for //a:a //b:b");
    }

    struct FakeQuery {
        outputs: RefCell<Vec<QueryResult>>,
    }

    impl FakeQuery {
        fn new(outputs: Vec<QueryResult>) -> Self {
            FakeQuery {
                outputs: RefCell::new(outputs),
            }
        }

        fn ok(lines: &str) -> QueryResult {
            QueryResult {
                code: Some(0),
                stdout: lines.as_bytes().to_vec(),
                stderr: Vec::new(),
            }
        }
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    #[test]
    fn shuffled_query_orders_yield_identical_build_argv() {
        let scratch = dx_test_scratch::scratch("dx-quality-query-order-");
        let root: PathBuf = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("pkg")).expect("pkg dir");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("BUILD file");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source file");
        let forward = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n//pkg:extra\n")]);
        let reversed = FakeQuery::new(vec![FakeQuery::ok("//pkg:extra\n//pkg:lib\n")]);
        let first = resolve(&strings(&["pkg/a.py"]), &root, &forward, &[]).expect("forward");
        let second = resolve(&strings(&["pkg/a.py"]), &root, &reversed, &[]).expect("reversed");
        assert_eq!(first.targets, second.targets);
        assert_eq!(first.targets, strings(&["//pkg:extra", "//pkg:lib"]));
        let first_plan =
            plan_build(Command::Lint, &first, &[], "/tmp/bep.json", &[]).expect("forward plan");
        let second_plan =
            plan_build(Command::Lint, &second, &[], "/tmp/bep.json", &[]).expect("reversed plan");
        assert_eq!(first_plan.argv, second_plan.argv);
    }

    #[test]
    fn typecheck_plan_carries_typecheck_aspect_and_format_summary() {
        let plan = plan_build(
            Command::Typecheck,
            &resolved(&[]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg
                .contains("//quality:real_aspects.bzl%real_typecheck_aspect")
                && arg.contains("//quality:real_aspects.bzl%real_rust_typecheck_aspect")),
            "typecheck selects the real typecheck aspects: {plan:?}"
        );
        assert_eq!(plan.summary, "Running typecheck analysis for //...");
        let plan =
            plan_build(Command::Format, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
        assert_eq!(plan.summary, "Running format analysis for //...");
    }

    #[test]
    fn keep_going_repetition_is_accepted() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--keep_going"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(plan.argv.iter().any(|arg| arg == "--keep_going"));
    }

    #[test]
    fn consumer_policy_selection_reaches_bazel() {
        let selection = "--@rules_dx//config:workspace=//consumer:policy";
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&[selection]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert_eq!(
            plan.argv.iter().filter(|arg| *arg == selection).count(),
            1,
            "the consumer policy selection passes through once: {plan:?}"
        );
    }

    #[test]
    fn conflicting_workflow_options_fail_before_execution() {
        for conflicting in [
            "--aspects=//other.bzl%aspect",
            "--output_groups=other",
            "--@rules_dx//config:validate=true",
            "--nokeep_going",
            "--build_event_json_file=/tmp/other.json",
            "--@rules_rust//rust/settings:clippy_output_diagnostics=false",
            "--@rules_rust//rust/settings:clippy_output_diagnostics",
        ] {
            let err = plan_build(
                Command::Lint,
                &resolved(&[]),
                &strings(&[conflicting]),
                "/tmp/bep.json",
                &[],
            )
            .expect_err("conflict must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
        for conflicting in [
            "--@rules_rust//rust/settings:rustc_output_diagnostics=false",
            "--@rules_rust//rust/settings:rustc_output_diagnostics",
        ] {
            let err = plan_build(
                Command::Typecheck,
                &resolved(&[]),
                &strings(&[conflicting]),
                "/tmp/bep.json",
                &[],
            )
            .expect_err("conflict must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
    }

    #[test]
    fn startup_options_are_rejected_as_command_options() {
        let err = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--home_rc"]),
            "/tmp/bep.json",
            &[],
        )
        .expect_err("startup option must fail");
        assert!(matches!(err, ForwardError::StartupOption { .. }));
    }

    #[test]
    fn unsupported_report_format_fails_with_command_registry() {
        let invocation = parse(&strings(&["lint", "--report=junit=out.xml"])).expect("parse");
        let entry = spec(invocation.command);
        assert!(
            !entry.accepts_report(&invocation.reports[0].format),
            "junit is not a lint report: {invocation:?}"
        );
        assert_eq!(
            parse(&strings(&["format", "--report=sarif=out.sarif"])),
            Err(crate::args::ArgsError::UnsupportedOption {
                command: "format",
                option: "--report=sarif=out.sarif".to_owned(),
            })
        );
        let words = ["lint", "--report=sarif"];
        let error = parse(&strings(&words)).unwrap_err();
        let ArgsError::Usage { text } = &error else {
            panic!("--report=sarif: want Usage, got {error:?}");
        };
        assert!(text.contains("report"), "{text}");
    }
}
