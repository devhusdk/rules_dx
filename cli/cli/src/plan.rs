pub mod generate;
pub mod managed;
pub mod quality;
pub mod registry;
pub mod run_deploy;
pub mod run_temp;
pub mod workflow;

use std::path::Path;
use std::sync::OnceLock;

use dx_process::{ForwardError, ProtectedFlag};

pub use generate::{
    generate_package_scope, generate_scope_elements, generate_scope_json, generate_traversal_dirs,
    plan_generate, GenerateScopeElement, GENERATE_CHECK_TARGET, GENERATE_ENV_INTENDED,
    GENERATE_ENV_MODE, GENERATE_ENV_SCOPE, GENERATE_TARGET,
};
pub use managed::{plan_managed, plan_managed_with_roots};
pub(crate) use quality::workflow_scope_labels;
pub use quality::{plan_build, plan_build_with, plan_probe, protected_flags, required_options};
pub use registry::{spec, CommandSpec};
pub use run_deploy::{
    plan_deploy_build, plan_deploy_run, plan_run, plan_run_build, plan_run_targets, shell_join,
    shell_quote,
};
pub use run_temp::{
    bep_path, clean_for_stop, create_run_temp_dir, intended_path, patch_path, remove_on_stop,
    remove_stale_run_dirs, results_path, run_id, run_nonce, RunTempDir,
};
pub use workflow::{plan_workflow, workflow_options, workflow_protected, WorkflowVerb};

pub const CLIPPY_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:clippy_output_diagnostics=true";

pub const RUSTC_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:rustc_output_diagnostics=true";

pub const BAZEL_SETTINGS: [&str; 2] = [CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG];

const UPSTREAM_SETTINGS: [&str; 2] = [
    "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
    "--@rules_rust//rust/settings:rustc_output_diagnostics=true",
];

const RULES_DX: &str = "rules_dx";

static RULES_DX_REPO: OnceLock<String> = OnceLock::new();

/// Records the name the workspace MODULE.bazel gives rules_dx.
pub fn set_rules_dx_repo(workspace: &Path) {
    let repo = std::fs::read_to_string(workspace.join("MODULE.bazel"))
        .map_or_else(|_| RULES_DX.to_owned(), |text| rules_dx_repo_of(&text));
    let _ = RULES_DX_REPO.set(repo);
}

/// Returns the recorded name for rules_dx, or `rules_dx`.
pub(crate) fn rules_dx_repo() -> &'static str {
    RULES_DX_REPO.get().map_or(RULES_DX, String::as_str)
}

/// Returns the name a MODULE.bazel source gives rules_dx.
pub fn rules_dx_repo_of(module_text: &str) -> String {
    let Ok(file) = dx_module_bazel::ModuleFile::parse(module_text) else {
        return RULES_DX.to_owned();
    };
    let root = file
        .calls_named("module")
        .find(|call| call.str("name") == Some(RULES_DX));
    let repo = match root {
        Some(call) => call.str("repo_name").unwrap_or(RULES_DX),
        None => file.repo_name(RULES_DX).unwrap_or(RULES_DX),
    };
    if valid_repo_name(repo) {
        repo.to_owned()
    } else {
        RULES_DX.to_owned()
    }
}

fn valid_repo_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Returns a label in the rules_dx repository.
pub fn rules_dx_label(rest: &str) -> String {
    format!("@{}//{rest}", rules_dx_repo())
}

/// Returns the shared settings for a Bazel command, spelled with `repo`.
pub fn settings_for(repo: &str, bazel_command: &str) -> Vec<String> {
    match bazel_command {
        "build" | "test" | "coverage" | "run" | "cquery" | "aquery" => BAZEL_SETTINGS
            .iter()
            .map(|setting| setting.replacen("@rules_dx//", &format!("@{repo}//"), 1))
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the settings every build-like Bazel command shares.
pub fn shared_settings(bazel_command: &str) -> Vec<String> {
    settings_for(rules_dx_repo(), bazel_command)
}

/// Puts the profile config first so user options can override it.
pub(crate) fn profile_options(
    profile: Option<crate::args::Profile>,
    options: &[String],
) -> Vec<String> {
    let mut leading = Vec::with_capacity(options.len() + 1);
    if let Some(profile) = profile {
        leading.push(profile.config_flag());
    }
    leading.extend(options.iter().cloned());
    leading
}

/// Builds a Bazel argv with the shared settings and their guards.
#[allow(clippy::disallowed_methods)]
pub fn bazel_argv(
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, ForwardError> {
    let settings = shared_settings(bazel_command);
    let mut required = settings.clone();
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

/// The output group the shard probe writes `{"shards":[...]}` files to.
pub const SHARD_OUTPUT_GROUP: &str = "dx_shards";

pub const KEEP_GOING_FLAG: &str = "--keep_going";

pub const SKIP_INCOMPATIBLE_FLAG: &str = "--skip_incompatible_explicit_targets";

pub const DOWNLOAD_ALL_FLAG: &str = "--remote_download_outputs=all";

pub const DOWNLOAD_TOPLEVEL_FLAG: &str = "--remote_download_outputs=toplevel";

/// The download mode for every command that reads outputs: all if the user's last choice was
/// all, else toplevel. Minimal is never allowed, since dx reads reports from the outputs.
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

/// Guards that keep user flags from selecting a download mode dx cannot read reports under.
/// The bare `--remote_download_toplevel` and `--remote_download_all` forms are unguarded.
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

/// The flags that write the build event file with local file paths.
pub(crate) fn bep_options(path: &str) -> Vec<String> {
    vec![
        format!("--{BEP_FLAG_NAME}={path}"),
        BEP_PATH_CONVERSION_FLAG.to_owned(),
    ]
}

/// Guards that keep user flags from moving or rewriting the build event file.
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
    use super::{rules_dx_label, rules_dx_repo_of, settings_for, BAZEL_SETTINGS};

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
        for verb in ["build", "test", "coverage", "run", "cquery", "aquery"] {
            assert_eq!(settings_for("rules_dx", verb), BAZEL_SETTINGS, "{verb}");
        }
        assert!(settings_for("my_dx", "query").is_empty());
        assert!(settings_for("my_dx", "fetch").is_empty());
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
