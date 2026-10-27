use dx_process::{ForwardError, ProtectedFlag};

use super::{bep_guards, bep_options, BuildPlan};
use crate::args::Command;

pub fn plan_managed(
    command: Command,
    scope: &dx_setup::SetupScope,
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
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
        _ => return Err(unsupported()),
    };
    let argv = managed_argv(command, &roots, bazel_options, bep_path, startup_options)?;
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
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let argv = managed_argv(command, roots, bazel_options, bep_path, startup_options)?;
    let summary = format!("Running {} for {}", command.name(), roots.join(" "));
    Ok(BuildPlan { argv, summary })
}

fn managed_argv(
    command: Command,
    roots: &[String],
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
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
        _ => return Err(unsupported()),
    };
    let mut required = Vec::with_capacity(aspects.len() + output_groups.len() + 3);
    for aspect in &aspects {
        required.push(format!("--aspects={aspect}"));
    }
    for group in &output_groups {
        required.push(format!("--output_groups={group}"));
    }
    required.push(super::download_flag(bazel_options).to_owned());
    required.extend(bep_options(bep_path));
    let mut protected = vec![
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
    ];
    protected.extend(super::download_guards());
    protected.extend(bep_guards());
    super::bazel_argv(
        "build",
        bazel_options,
        &required,
        &protected,
        roots,
        startup_options,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_testing::cli::strings;

    #[test]
    fn managed_plan_builds_collection_request() {
        use dx_setup::SetupScope;

        let plan = plan_managed(
            Command::Codegen,
            &SetupScope::Repository,
            &strings(&["--jobs=4"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--jobs=4",
                crate::plan::CLIPPY_DIAGNOSTICS_FLAG,
                crate::plan::RUSTC_DIAGNOSTICS_FLAG,
                "--aspects=//generation:codegen.bzl%dx_codegen_plan_aspect",
                "--output_groups=dx_codegen_plans",
                "--remote_download_outputs=toplevel",
                "--build_event_json_file=/tmp/bep.json",
                "--build_event_json_file_path_conversion=false",
                "//dx:codegen",
            ])
        );
        assert_eq!(plan.summary, "Running codegen for //...");
        let plan = plan_managed(
            Command::Env,
            &SetupScope::Repository,
            &[],
            "/tmp/bep.json",
            &[],
        )
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
    }

    #[test]
    fn managed_plan_exact_scope_selects_label() {
        use dx_setup::SetupScope;

        for command in [Command::Codegen, Command::Env] {
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
        let plan = plan_managed_with_roots(Command::Codegen, &roots, &[], "/tmp/bep.json", &[])
            .expect("plan");
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
        for command in [Command::Build, Command::Lint] {
            let err = plan_managed_with_roots(command, &roots, &[], "/tmp/bep.json", &[])
                .expect_err("unmanaged must fail");
            assert!(
                matches!(err, ForwardError::UnsupportedCommand { .. }),
                "{command:?} produced {err:?}"
            );
        }
    }

    fn last_download_flag(argv: &[String]) -> Option<&str> {
        argv.iter()
            .rev()
            .map(String::as_str)
            .find(|arg| arg.starts_with("--remote_download"))
    }

    #[test]
    fn managed_plan_forces_toplevel_download_after_user_config() {
        use dx_setup::SetupScope;

        let options = strings(&["--config=remote", "--jobs=4"]);
        let roots = strings(&["//a:one"]);
        for command in [Command::Codegen, Command::Env] {
            let plans = [
                plan_managed(
                    command,
                    &SetupScope::Repository,
                    &options,
                    "/tmp/bep.json",
                    &[],
                )
                .expect("plan"),
                plan_managed_with_roots(command, &roots, &options, "/tmp/bep.json", &[])
                    .expect("plan"),
            ];
            for plan in plans {
                let config = plan
                    .argv
                    .iter()
                    .position(|arg| arg == "--config=remote")
                    .expect("config kept");
                let download = plan
                    .argv
                    .iter()
                    .position(|arg| arg == crate::plan::DOWNLOAD_TOPLEVEL_FLAG)
                    .expect("toplevel forced");
                assert!(config < download, "{command:?}: {plan:?}");
                assert_eq!(
                    last_download_flag(&plan.argv),
                    Some(crate::plan::DOWNLOAD_TOPLEVEL_FLAG),
                    "{command:?}: {plan:?}"
                );
            }
        }
    }

    #[test]
    fn managed_plan_accepts_options_that_keep_its_outputs() {
        use dx_setup::SetupScope;

        for (option, expected) in [
            (
                "--remote_download_outputs=toplevel",
                crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            ),
            (
                "--remote_download_toplevel",
                crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            ),
            (
                "--remote_download_outputs=all",
                crate::plan::DOWNLOAD_ALL_FLAG,
            ),
            ("--remote_download_all", crate::plan::DOWNLOAD_ALL_FLAG),
            (
                "--nobuild_event_json_file_path_conversion",
                crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            ),
            (
                "--build_event_json_file_path_conversion=false",
                crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            ),
        ] {
            let plan = plan_managed(
                Command::Codegen,
                &SetupScope::Repository,
                &strings(&[option]),
                "/tmp/bep.json",
                &[],
            )
            .expect("download mode keeps outputs");
            assert!(plan.argv.iter().any(|arg| arg == option), "{plan:?}");
            assert_eq!(last_download_flag(&plan.argv), Some(expected), "{plan:?}");
        }
        let plan = plan_managed(
            Command::Env,
            &SetupScope::Repository,
            &strings(&["--remote_download_all", "--remote_download_toplevel"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("last download choice wins");
        assert_eq!(
            last_download_flag(&plan.argv),
            Some(crate::plan::DOWNLOAD_TOPLEVEL_FLAG),
            "{plan:?}"
        );
    }

    #[test]
    fn managed_plan_rejects_policy_conflicts_and_startup_options() {
        use dx_setup::SetupScope;

        for conflicting in [
            "--aspects=//other.bzl%aspect",
            "--output_groups=other",
            "--build_event_json_file=/tmp/other.json",
            "--build_event_json_file_path_conversion",
            "--build_event_json_file_path_conversion=true",
            "--remote_download_minimal",
            "--remote_download_outputs=minimal",
        ] {
            let err = plan_managed(
                Command::Codegen,
                &SetupScope::Repository,
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
        let err = plan_managed(
            Command::Codegen,
            &SetupScope::Repository,
            &strings(&["--home_rc"]),
            "/tmp/bep.json",
            &[],
        )
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
}
