pub mod generate;
pub mod managed;
pub mod quality;
pub mod registry;
pub mod run_deploy;
pub mod run_temp;
pub mod workflow;

use dx_error::DxError;
use std::path::Path;
use std::sync::OnceLock;

use dx_process::ProtectedFlag;

pub use generate::{
    GENERATE_ENV_INTENDED, GENERATE_ENV_MODE, GENERATE_ENV_SCOPE, generate_package_scope,
    generate_package_scope_within, generate_scope_json, generate_scope_visits, plan_generate,
};
pub use managed::{managed_aspects, plan_managed, plan_managed_with_roots};
pub use quality::{
    Selection, command_aspects, plan_build, plan_build_with, plan_docs, plan_probe,
    plan_shared_build, plan_shared_probe,
};
pub(crate) use quality::{workflow_bazel_labels, workflow_scope_labels};
pub use registry::{Consumer, spec};
pub use run_deploy::{
    plan_deploy_build, plan_deploy_run, plan_run, plan_run_build, plan_run_targets, shell_join,
    shell_quote,
};
pub use run_temp::{
    bep_path, clean_for_stop, create_run_temp_dir, intended_path, patch_path, remove_on_stop,
    remove_stale_run_dirs, results_path, run_id,
};
pub use workflow::{WorkflowVerb, plan_workflow, workflow_protected};

pub use dx_process::{
    BAZEL_SETTINGS, CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG, head_settings,
};

const UPSTREAM_SETTINGS: [&str; 2] = [
    "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
    "--@rules_rust//rust/settings:rustc_output_diagnostics=true",
];

const RULES_DX: &str = "rules_dx";

static RULES_DX_REPO: OnceLock<String> = OnceLock::new();

static LAYER_SETS_DIAGNOSTICS: OnceLock<bool> = OnceLock::new();

pub fn set_rules_dx_repo(workspace: &Path) {
    let repo = dx_module_bazel::rules_dx_repo(workspace);
    LAYER_SETS_DIAGNOSTICS
        .set(correctness_layer_imported(workspace, &repo))
        .unwrap_or_default();
    drop(RULES_DX_REPO.set(repo));
}

#[must_use]
pub fn correctness_layer_imported(workspace: &Path, repo: &str) -> bool {
    let rc_text = std::fs::read_to_string(workspace.join(".bazelrc")).unwrap_or_default();
    [dx_preset::CONSUMER_DIR].into_iter().any(|dir| {
        let imported = dx_preset::layer_import_state(&rc_text, dir)
            .into_iter()
            .any(|(layer, state)| {
                layer == dx_preset::Layer::Correctness && state == dx_preset::ImportState::Active
            });
        imported
            && std::fs::read_to_string(
                workspace
                    .join(dir)
                    .join(dx_preset::Layer::Correctness.file_name()),
            )
            .is_ok_and(|text| {
                dx_preset::layer_is_current(&text, dx_preset::Layer::Correctness, repo)
            })
    })
}

pub(crate) fn rules_dx_repo() -> &'static str {
    RULES_DX_REPO.get().map_or(RULES_DX, String::as_str)
}

#[must_use]
pub fn rules_dx_repo_of(module_text: &str) -> String {
    dx_module_bazel::rules_dx_repo_of(module_text)
}

#[must_use]
pub fn rules_dx_label(rest: &str) -> String {
    format!("@{}//{rest}", rules_dx_repo())
}

#[must_use]
pub fn settings_for(repo: &str, bazel_command: &str) -> Vec<String> {
    settings_in(repo, bazel_command, layer_sets_diagnostics())
}

fn layer_sets_diagnostics() -> bool {
    LAYER_SETS_DIAGNOSTICS.get().copied().unwrap_or(false)
}

fn takes_settings(bazel_command: &str) -> bool {
    dx_process::takes_shared_settings(bazel_command) || matches!(bazel_command, "fetch" | "vendor")
}

fn settings_in(repo: &str, bazel_command: &str, layered: bool) -> Vec<String> {
    if takes_settings(bazel_command) && !layered {
        head_settings(repo)
    } else {
        Vec::new()
    }
}

#[must_use]
pub fn shared_settings(bazel_command: &str) -> Vec<String> {
    settings_for(rules_dx_repo(), bazel_command)
}

pub(crate) fn build_mode_options(
    build_mode: Option<crate::args::BuildMode>,
    options: &[String],
) -> Vec<String> {
    let mut leading = Vec::with_capacity(options.len() + 1);
    if let Some(build_mode) = build_mode {
        leading.push(build_mode.config_flag());
    }
    leading.extend(options.iter().cloned());
    leading
}

pub fn bazel_argv(
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, DxError> {
    bazel_argv_in(
        rules_dx_repo(),
        bazel_command,
        user_options,
        required_options,
        protected,
        labels,
        startup_options,
        layer_sets_diagnostics(),
    )
}

#[expect(
    clippy::disallowed_methods,
    clippy::too_many_arguments,
    reason = "the argv carries no build settings; each argument is a distinct input"
)]
fn bazel_argv_in(
    repo: &str,
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
    layered: bool,
) -> Result<Vec<String>, DxError> {
    let settings = if takes_settings(bazel_command) {
        head_settings(repo)
    } else {
        Vec::new()
    };
    let mut required = settings_in(repo, bazel_command, layered);
    required.extend(required_options.iter().cloned());
    let mut guards = protected.to_vec();
    let upstream: &[&str] = if settings.is_empty() {
        &[]
    } else {
        &UPSTREAM_SETTINGS
    };
    for setting in settings
        .iter()
        .map(String::as_str)
        .chain(upstream.iter().copied())
    {
        guards.push(ProtectedFlag {
            name: quality::setting_name(setting)?,
            required: Some(setting.to_owned()),
            allowed: Vec::new(),
        });
    }
    dx_process::build_workflow_argv(
        bazel_command,
        user_options,
        &required,
        &guards,
        labels,
        startup_options,
    )
}

pub const OUTPUT_GROUP: &str = "dx_results";

pub const DOCS_SITE_OUTPUT_GROUP: &str = "dx_docs_sites";

pub const DOCS_SHARD_OUTPUT_GROUP: &str = "dx_docs_shards";

pub const DOCS_TOOLS_OUTPUT_GROUP: &str = "dx_docs_tools";

pub const SKIP_OUTPUT_GROUP: &str = "dx_skips";
pub const LINK_REQUEST_OUTPUT_GROUP: &str = "dx_link_requests";

pub const SHARD_OUTPUT_GROUP: &str = "dx_shards";

pub const KEEP_GOING_FLAG: &str = "--keep_going";

pub const SKIP_INCOMPATIBLE_FLAG: &str = "--skip_incompatible_explicit_targets";

pub const DOWNLOAD_ALL_FLAG: &str = "--remote_download_outputs=all";

pub const DOWNLOAD_TOPLEVEL_FLAG: &str = "--remote_download_outputs=toplevel";

pub(crate) fn download_flag(user_options: &[String]) -> &'static str {
    let mut flag = DOWNLOAD_TOPLEVEL_FLAG;
    for option in user_options {
        match option.as_str() {
            DOWNLOAD_ALL_FLAG | "--remote_download_all" => flag = DOWNLOAD_ALL_FLAG,
            DOWNLOAD_TOPLEVEL_FLAG | "--remote_download_toplevel" => flag = DOWNLOAD_TOPLEVEL_FLAG,
            _ => {}
        }
    }
    flag
}

pub(crate) fn download_guards() -> Vec<ProtectedFlag> {
    vec![
        ProtectedFlag {
            name: "remote_download_outputs".to_owned(),
            required: None,
            allowed: vec![
                DOWNLOAD_TOPLEVEL_FLAG.to_owned(),
                DOWNLOAD_ALL_FLAG.to_owned(),
            ],
        },
        ProtectedFlag {
            name: "remote_download_minimal".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ]
}

pub const BEP_FLAG_NAME: &str = "build_event_json_file";

pub const BEP_PATH_CONVERSION_FLAG: &str = "--build_event_json_file_path_conversion=false";

pub(crate) fn bep_options(path: &str) -> Vec<String> {
    vec![
        format!("--{BEP_FLAG_NAME}={path}"),
        BEP_PATH_CONVERSION_FLAG.to_owned(),
    ]
}

pub(crate) fn bep_guards() -> Vec<ProtectedFlag> {
    vec![
        ProtectedFlag {
            name: BEP_FLAG_NAME.to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "build_event_json_file_path_conversion".to_owned(),
            required: Some(BEP_PATH_CONVERSION_FLAG.to_owned()),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "nobuild_event_json_file_path_conversion".to_owned(),
            required: None,
            allowed: vec!["--nobuild_event_json_file_path_conversion".to_owned()],
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    pub argv: Vec<String>,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::{
        BAZEL_SETTINGS, bazel_argv_in, correctness_layer_imported, rules_dx_label,
        rules_dx_repo_of, settings_for,
    };

    fn workspace(rc: &str, layer: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".bazelrc"), rc).expect("write rc");
        if let Some(text) = layer {
            let layers = dir.path().join(".dx/bazelrc");
            std::fs::create_dir_all(&layers).expect("mkdir");
            std::fs::write(layers.join("correctness.bazelrc"), text).expect("write layer");
        }
        dir
    }

    fn current_layer() -> String {
        dx_preset::render_layer(dx_preset::Layer::Correctness, "rules_dx")
    }

    #[test]
    fn an_imported_current_layer_stands_in_for_the_flags() {
        let rc = "try-import %workspace%/.dx/bazelrc/correctness.bazelrc\n";
        let layer = current_layer();
        let dir = workspace(rc, Some(&layer));
        assert!(correctness_layer_imported(dir.path(), "rules_dx"));
        let argv = bazel_argv_in("rules_dx", "build", &[], &[], &[], &[], &[], true).expect("plan");
        for setting in BAZEL_SETTINGS {
            assert!(!argv.contains(&setting.to_owned()), "{setting} in {argv:?}");
        }
        let conflicting = vec!["--@rules_dx//config:clippy_output_diagnostics=false".to_owned()];
        let error = bazel_argv_in("rules_dx", "build", &conflicting, &[], &[], &[], &[], true)
            .expect_err("conflict");
        assert!(error.message().starts_with("conflicting_option: --"));
    }

    #[test]
    fn a_commented_out_or_stale_layer_keeps_the_flags() {
        let layer = current_layer();
        let commented = workspace(
            "# try-import %workspace%/.dx/bazelrc/correctness.bazelrc\n",
            Some(&layer),
        );
        assert!(!correctness_layer_imported(commented.path(), "rules_dx"));
        let rc = "try-import %workspace%/.dx/bazelrc/correctness.bazelrc\n";
        let stale = workspace(rc, Some("build --something_else\n"));
        assert!(!correctness_layer_imported(stale.path(), "rules_dx"));
        let argv =
            bazel_argv_in("rules_dx", "build", &[], &[], &[], &[], &[], false).expect("plan");
        for setting in BAZEL_SETTINGS {
            assert!(
                argv.contains(&setting.to_owned()),
                "{setting} missing from {argv:?}"
            );
        }
    }

    #[test]
    fn settings_use_the_given_repo_name() {
        assert_eq!(
            settings_for("my_dx", "cquery"),
            [
                "--@my_dx//config:clippy_output_diagnostics=true",
                "--@my_dx//config:rustc_output_diagnostics=true",
            ]
        );
    }

    #[test]
    fn default_settings_match_the_rules_dx_spelling() {
        for verb in dx_process::SHARED_SETTINGS_VERBS {
            assert_eq!(settings_for("rules_dx", verb), BAZEL_SETTINGS, "{verb}");
        }
        assert_eq!(settings_for("my_dx", "query"), [] as [String; 0]);
        assert_eq!(settings_for("my_dx", "mod"), [] as [String; 0]);
        assert_eq!(settings_for("rules_dx", "fetch"), BAZEL_SETTINGS);
        assert_eq!(settings_for("rules_dx", "vendor"), BAZEL_SETTINGS);
    }

    #[test]
    fn repo_name_comes_from_the_rules_dx_bazel_dep() {
        assert_eq!(
            rules_dx_repo_of(
                "module(name = \"consumer\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\n"
            ),
            "my_dx"
        );
        assert_eq!(
            rules_dx_repo_of(
                "module(name = \"consumer\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n"
            ),
            "rules_dx"
        );
        assert_eq!(
            rules_dx_repo_of("module(name = \"consumer\")\n"),
            "rules_dx"
        );
    }

    #[test]
    fn root_module_uses_its_own_repo_name() {
        assert_eq!(
            rules_dx_repo_of("module(name = \"rules_dx\", repo_name = \"dx\")\n"),
            "dx"
        );
        assert_eq!(
            rules_dx_repo_of("module(name = \"rules_dx\")\n"),
            "rules_dx"
        );
        assert_eq!(
            rules_dx_repo_of("module(name = \"fixture\", repo_name = \"rules_dx\")\n"),
            "rules_dx"
        );
    }

    #[test]
    fn renamed_repo_settings_are_spelled_and_guarded_with_that_repo() {
        let plan =
            |user: &[String]| bazel_argv_in("my_dx", "build", user, &[], &[], &[], &[], false);
        let argv = plan(&[]).expect("plan");
        for setting in BAZEL_SETTINGS {
            let renamed = setting.replacen("@rules_dx//", "@my_dx//", 1);
            assert!(argv.contains(&renamed), "{renamed} missing from {argv:?}");
            assert!(!argv.contains(&setting.to_owned()), "{setting} in {argv:?}");
        }
        for conflicting in [
            "--@my_dx//config:clippy_output_diagnostics=false",
            "--@my_dx//config:rustc_output_diagnostics=false",
        ] {
            let err = plan(&[conflicting.to_owned()]).expect_err("conflicting setting must fail");
            assert!(
                err.message().starts_with("conflicting_option: --"),
                "{conflicting} produced {err:?}"
            );
        }
    }

    #[test]
    fn labels_default_to_the_rules_dx_repo() {
        assert_eq!(
            rules_dx_label("config:workspace"),
            "@rules_dx//config:workspace"
        );
    }

    #[test]
    fn bad_input_falls_back_to_rules_dx() {
        assert_eq!(rules_dx_repo_of("bazel_dep(name = "), "rules_dx");
        for bad in ["my dx", "a//b", "", "1dx", "@dx"] {
            let text = format!(
                "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"{bad}\")\n"
            );
            assert_eq!(rules_dx_repo_of(&text), "rules_dx", "{bad:?}");
        }
    }
}
