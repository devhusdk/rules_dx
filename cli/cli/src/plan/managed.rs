use dx_process::{build_workflow_argv, ForwardError, ProtectedFlag};

use super::{BuildPlan, BEP_FLAG_NAME};
use crate::args::Command;

pub fn plan_bazel(forwarded: &[String]) -> BuildPlan {
    let mut argv = Vec::with_capacity(1 + forwarded.len());
    argv.push(dx_process::launcher_argv0().to_owned());
    argv.extend(forwarded.iter().cloned());
    let summary = if forwarded.is_empty() {
        "Running bazel".to_owned()
    } else {
        format!("Running bazel {}", forwarded.join(" "))
    };
    BuildPlan { argv, summary }
}

pub fn plan_managed(
    command: Command,
    scope: &dx_setup::SetupScope,
    bazel_options: &[String],
    bep_path: &str,
    startup: &[String],
) -> Result<BuildPlan, ForwardError> {
    let unsupported = || ForwardError::UnsupportedCommand {
        command: command.name().to_owned(),
    };
    if !command.is_managed() {
        return Err(unsupported());
    }
    let roots = match command {
        Command::Codegen => {
            let scope = match scope {
                dx_setup::SetupScope::Repository => dx_codegen::CodegenScope::Repository,
                dx_setup::SetupScope::Exact(label) => {
                    dx_codegen::CodegenScope::Exact(label.clone())
                }
            };
            dx_codegen::scope_targets(&scope)
        }
        Command::Env => {
            let scope = match scope {
                dx_setup::SetupScope::Repository => dx_env_plan::EnvScope::Repository,
                dx_setup::SetupScope::Exact(label) => dx_env_plan::EnvScope::Exact(label.clone()),
            };
            dx_env_plan::scope_targets(&scope)
        }
        Command::Setup => dx_setup::plan_request(scope).roots,
        _ => return Err(unsupported()),
    };
    let argv = managed_argv(command, &roots, bazel_options, bep_path, startup)?;
    let display = match scope {
        dx_setup::SetupScope::Repository => "//...",
        dx_setup::SetupScope::Exact(label) => label,
    };
    let summary = format!("Running {} for {}", command.name(), display);
    Ok(BuildPlan { argv, summary })
}

pub fn plan_managed_with_roots(
    command: Command,
    roots: &[String],
    bazel_options: &[String],
    bep_path: &str,
    startup: &[String],
) -> Result<BuildPlan, ForwardError> {
    let argv = managed_argv(command, roots, bazel_options, bep_path, startup)?;
    let summary = format!("Running {} for {}", command.name(), roots.join(" "));
    Ok(BuildPlan { argv, summary })
}

fn managed_argv(
    command: Command,
    roots: &[String],
    bazel_options: &[String],
    bep_path: &str,
    startup: &[String],
) -> Result<Vec<String>, ForwardError> {
    let unsupported = || ForwardError::UnsupportedCommand {
        command: command.name().to_owned(),
    };
    if !command.is_managed() {
        return Err(unsupported());
    }
    let (aspects, output_groups) = match command {
        Command::Codegen => (
            vec![dx_codegen::CODEGEN_ASPECT.to_owned()],
            vec![dx_codegen::OUTPUT_GROUP.to_owned()],
        ),
        Command::Env => (
            vec![dx_env_plan::ENV_ASPECT.to_owned()],
            vec![dx_env_plan::OUTPUT_GROUP.to_owned()],
        ),
        Command::Setup => (
            vec![
                dx_setup::CODEGEN_ASPECT.to_owned(),
                dx_setup::ENV_ASPECT.to_owned(),
            ],
            vec![
                dx_setup::CODEGEN_OUTPUT_GROUP.to_owned(),
                dx_setup::ENV_OUTPUT_GROUP.to_owned(),
            ],
        ),
        _ => return Err(unsupported()),
    };
    let mut required = Vec::with_capacity(aspects.len() + output_groups.len() + 1);
    for aspect in &aspects {
        required.push(format!("--aspects={aspect}"));
    }
    for group in &output_groups {
        required.push(format!("--output_groups={group}"));
    }
    required.push(format!("--{BEP_FLAG_NAME}={bep_path}"));
    let protected = vec![
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
            name: BEP_FLAG_NAME.to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ];
    build_workflow_argv("build", bazel_options, &required, &protected, roots, startup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;

    #[test]
    fn managed_plan_builds_collection_request() {
        use dx_setup::SetupScope;

        let plan = plan_managed(
            Command::Codegen,
            &SetupScope::Repository,
            &strings(&["--jobs=4"]),
            "/tmp/bep.json",
        &[], )
        .expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--aspects=//generation:codegen.bzl%dx_codegen_plan_aspect",
                "--output_groups=dx_codegen_plans",
                "--build_event_json_file=/tmp/bep.json",
                "--jobs=4",
                "//dx:codegen",
            ])
        );
        assert_eq!(plan.summary, "Running codegen for //...");
        let plan = plan_managed(Command::Env, &SetupScope::Repository, &[], "/tmp/bep.json", &[])
            .expect("plan");
        assert!(plan
            .argv
            .iter()
            .any(|arg| arg == "--aspects=//env:plan.bzl%dx_env_plan_aspect"));
        assert!(plan
            .argv
            .iter()
            .any(|arg| arg == "--output_groups=dx_env_plans"));
        assert_eq!(plan.argv.last(), Some(&"//dx:env".to_owned()));
        assert_eq!(plan.summary, "Running env for //...");
        let plan = plan_managed(
            Command::Setup,
            &SetupScope::Repository,
            &[],
            "/tmp/bep.json",
        &[], )
        .expect("plan");
        assert_eq!(
            plan.argv
                .iter()
                .filter(|arg| arg.starts_with("--aspects="))
                .count(),
            2,
            "setup requests both collecting aspects: {plan:?}"
        );
        assert_eq!(
            plan.argv
                .iter()
                .filter(|arg| arg.starts_with("--output_groups="))
                .count(),
            2,
            "setup requests both output groups: {plan:?}"
        );
        assert!(plan.argv.contains(&"//dx:codegen".to_owned()));
        assert!(plan.argv.contains(&"//dx:env".to_owned()));
        assert_eq!(plan.summary, "Running setup for //...");
    }

    #[test]
    fn managed_plan_exact_scope_selects_label() {
        use dx_setup::SetupScope;

        for command in [Command::Codegen, Command::Env, Command::Setup] {
            let scope = SetupScope::Exact("//a:one".to_owned());
            let plan = plan_managed(command, &scope, &[], "/tmp/bep.json", &[]).expect("plan");
            assert_eq!(plan.argv.last(), Some(&"//a:one".to_owned()), "{command:?}");
            assert_eq!(
                plan.summary,
                format!("Running {} for //a:one", command.name())
            );
        }
    }

    #[test]
    fn managed_expanded_roots_join_every_root_in_argv_and_summary() {
        let roots = strings(&[
            "//generation:codegen_prost_fixture",
            "//generation:result_proto",
        ]);
        let plan =
            plan_managed_with_roots(Command::Codegen, &roots, &[], "/tmp/bep.json", &[]).expect("plan");
        assert_eq!(
            plan.argv.last(),
            Some(&"//generation:result_proto".to_owned())
        );
        assert!(plan
            .argv
            .contains(&"//generation:codegen_prost_fixture".to_owned()));
        assert_eq!(
            plan.summary,
            "Running codegen for //generation:codegen_prost_fixture //generation:result_proto"
        );
        let plan =
            plan_managed_with_roots(Command::Setup, &roots, &[], "/tmp/bep.json", &[]).expect("plan");
        assert_eq!(
            plan.argv
                .iter()
                .filter(|arg| arg.starts_with("--aspects="))
                .count(),
            2,
            "expanded setup still requests both aspects: {plan:?}"
        );
        assert_eq!(
            plan.summary,
            "Running setup for //generation:codegen_prost_fixture //generation:result_proto"
        );
        for command in [Command::Build, Command::Lint] {
            let err = plan_managed_with_roots(command, &roots, &[], "/tmp/bep.json", &[])
                .expect_err("unmanaged must fail");
            assert!(
                matches!(err, ForwardError::UnsupportedCommand { .. }),
                "{command:?} produced {err:?}"
            );
        }
    }

    #[test]
    fn managed_plan_rejects_policy_conflicts_and_startup_options() {
        use dx_setup::SetupScope;

        for conflicting in [
            "--aspects=//other.bzl%aspect",
            "--output_groups=other",
            "--build_event_json_file=/tmp/other.json",
        ] {
            let err = plan_managed(
                Command::Setup,
                &SetupScope::Repository,
                &strings(&[conflicting]),
                "/tmp/bep.json",
            &[], )
            .expect_err("conflict must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
        let err = plan_managed(
            Command::Codegen,
            &SetupScope::Repository,
            &strings(&["--home_rc"]),
            "/tmp/bep.json",
        &[], )
        .expect_err("startup option must fail");
        assert!(matches!(err, ForwardError::StartupOption { .. }));
    }

    #[test]
    fn managed_plan_rejects_unmanaged_commands() {
        use dx_setup::SetupScope;

        for command in [Command::Build, Command::Lint, Command::Run] {
            let err = plan_managed(command, &SetupScope::Repository, &[], "/tmp/bep.json", &[])
                .expect_err("unmanaged command must fail");
            assert!(
                matches!(err, ForwardError::UnsupportedCommand { .. }),
                "{command:?} produced {err:?}"
            );
        }
    }

    #[test]
    fn bazel_plan_forwards_arguments_verbatim() {
        let plan = plan_bazel(&strings(&["build", "//...", "--jobs=4"]));
        assert_eq!(
            plan.argv,
            strings(&["bazel", "build", "//...", "--jobs=4",])
        );
        assert!(plan.summary.contains("build //..."));
        let bare = plan_bazel(&[]);
        assert_eq!(bare.argv, strings(&["bazel"]));
    }
}
