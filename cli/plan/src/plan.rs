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
    generate_package_scope, generate_scope_json, plan_generate, plan_generate_logged,
    GENERATE_ENV_INTENDED, GENERATE_ENV_MODE, GENERATE_ENV_SCOPE,
};
pub use managed::{plan_managed, plan_managed_with_roots};
pub(crate) use quality::workflow_scope_labels;
pub use quality::{plan_build, plan_build_with, plan_probe};
pub use registry::spec;
pub use run_deploy::{
    plan_deploy_build, plan_deploy_run, plan_run, plan_run_build, plan_run_targets, shell_join,
    shell_quote,
};
pub use run_temp::{
    bep_path, clean_for_stop, create_run_temp_dir, intended_path, patch_path, remove_on_stop,
    remove_stale_run_dirs, results_path, run_id,
};
pub use workflow::{plan_workflow, workflow_protected, WorkflowVerb};

pub const CLIPPY_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:clippy_output_diagnostics=true";

pub const RUSTC_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:rustc_output_diagnostics=true";

pub const BAZEL_SETTINGS: [&str; 2] = [CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG];

const UPSTREAM_SETTINGS: [&str; 2] = [
    "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
    "--@rules_rust//rust/settings:rustc_output_diagnostics=true",
];

const RULES_DX: &str = "rules_dx";

static RULES_DX_REPO: OnceLock<String> = OnceLock::new();

/// Records the name the workspace MODULE.bazel (and the files it includes) gives rules_dx.
pub fn set_rules_dx_repo(workspace: &Path) {
    let read_include = |label: &str| {
        dx_module_bazel::label_path(label)
            .and_then(|rel| std::fs::read_to_string(workspace.join(&rel)).map_err(|_| rel))
            .map_err(|_| format!("workspace input {label}"))
    };
    let repo = std::fs::read_to_string(workspace.join("MODULE.bazel")).map_or_else(
        |_| RULES_DX.to_owned(),
        |text| rules_dx_repo_with(&text, &read_include),
    );
    let _ = RULES_DX_REPO.set(repo);
}

/// Returns the recorded name for rules_dx, or `rules_dx`.
pub(crate) fn rules_dx_repo() -> &'static str {
    RULES_DX_REPO.get().map_or(RULES_DX, String::as_str)
}

/// Returns the name a MODULE.bazel source gives rules_dx, ignoring any includes.
pub fn rules_dx_repo_of(module_text: &str) -> String {
    rules_dx_repo_with(module_text, &|_| Err("no includes".to_owned()))
}

/// Returns the name rules_dx gets across a MODULE.bazel and its includes, in include order.
/// When the includes cannot be loaded, falls back to the root file alone.
fn rules_dx_repo_with(
    module_text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> String {
    let repo = match dx_module_bazel::load_with_includes(module_text, read_include) {
        Ok(files) => files.iter().find_map(repo_in_file).map(str::to_owned),
        Err(_) => dx_module_bazel::ModuleFile::parse(module_text)
            .ok()
            .and_then(|file| repo_in_file(&file).map(str::to_owned)),
    };
    match repo {
        Some(repo) if valid_repo_name(&repo) => repo,
        _ => RULES_DX.to_owned(),
    }
}

/// The repo name one module file gives rules_dx, if the file declares rules_dx.
fn repo_in_file(file: &dx_module_bazel::ModuleFile) -> Option<&str> {
    match file
        .calls_named("module")
        .find(|call| call.str("name") == Some(RULES_DX))
    {
        Some(call) => Some(call.str("repo_name").unwrap_or(RULES_DX)),
        None => file.repo_name(RULES_DX),
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
    if dx_process::takes_shared_settings(bazel_command) {
        head_settings(repo)
    } else {
        Vec::new()
    }
}

/// The shared settings spelled with `repo`, for a `BazelHead`.
pub fn head_settings(repo: &str) -> Vec<String> {
    BAZEL_SETTINGS
        .iter()
        .map(|setting| setting.replacen("@rules_dx//", &format!("@{repo}//"), 1))
        .collect()
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
pub fn bazel_argv(
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, ForwardError> {
    bazel_argv_in(
        rules_dx_repo(),
        bazel_command,
        user_options,
        required_options,
        protected,
        labels,
        startup_options,
    )
}

/// Builds a Bazel argv whose shared settings are spelled with `repo`, and guards them.
#[allow(clippy::disallowed_methods)]
fn bazel_argv_in(
    repo: &str,
    bazel_command: &str,
    user_options: &[String],
    required_options: &[String],
    protected: &[ProtectedFlag],
    labels: &[String],
    startup_options: &[String],
) -> Result<Vec<String>, ForwardError> {
    let settings = settings_for(repo, bazel_command);
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

pub const EXEC_LOG_FLAG_NAME: &str = "execution_log_compact_file";

/// Every Bazel execution log flag, which cannot be combined with the compact log dx writes.
const EXEC_LOG_FLAG_NAMES: [&str; 4] = [
    EXEC_LOG_FLAG_NAME,
    "experimental_execution_log_compact_file",
    "execution_log_binary_file",
    "execution_log_json_file",
];

/// The option that writes the compact execution log to `path`.
pub fn exec_log_options(path: &str) -> Vec<String> {
    vec![format!("--{EXEC_LOG_FLAG_NAME}={path}")]
}

/// Guards that refuse user execution log flags while dx writes its own.
pub fn exec_log_guards() -> Vec<ProtectedFlag> {
    EXEC_LOG_FLAG_NAMES
        .iter()
        .map(|name| ProtectedFlag {
            name: (*name).to_owned(),
            required: None,
            allowed: Vec::new(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    pub argv: Vec<String>,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::{
        bazel_argv_in, rules_dx_label, rules_dx_repo_of, rules_dx_repo_with, settings_for,
        BAZEL_SETTINGS,
    };
    use dx_process::ForwardError;

    const DX_DEP: &str =
        "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\n";

    fn missing(label: &str) -> Result<String, String> {
        Err(format!("workspace input {label}"))
    }

    #[test]
    fn repo_name_comes_from_an_included_module_file() {
        let root = "module(name = \"consumer\")\ninclude(\"//deps:dx.MODULE.bazel\")\n";
        let read = |label: &str| match label {
            "//deps:dx.MODULE.bazel" => Ok(DX_DEP.to_owned()),
            _ => missing(label),
        };
        assert_eq!(rules_dx_repo_with(root, &read), "my_dx");
    }

    #[test]
    fn root_bazel_dep_wins_over_an_included_one() {
        let root = format!("{DX_DEP}include(\"//deps:dx.MODULE.bazel\")\n");
        let read = |_: &str| Ok("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n".to_owned());
        assert_eq!(rules_dx_repo_with(&root, &read), "my_dx");
    }

    #[test]
    fn nested_includes_are_followed() {
        let root = "module(name = \"consumer\")\ninclude(\"//deps:a.MODULE.bazel\")\n";
        let read = |label: &str| match label {
            "//deps:a.MODULE.bazel" => Ok("include(\"//deps:b.MODULE.bazel\")\n".to_owned()),
            "//deps:b.MODULE.bazel" => Ok(DX_DEP.to_owned()),
            _ => missing(label),
        };
        assert_eq!(rules_dx_repo_with(root, &read), "my_dx");
    }

    #[test]
    fn unreadable_include_falls_back_to_the_root_file() {
        let root = format!("{DX_DEP}include(\"//deps:gone.MODULE.bazel\")\n");
        assert_eq!(rules_dx_repo_with(&root, &missing), "my_dx");
        let bare = "module(name = \"consumer\")\ninclude(\"//deps:gone.MODULE.bazel\")\n";
        assert_eq!(rules_dx_repo_with(bare, &missing), "rules_dx");
    }

    #[test]
    fn included_repo_name_must_be_valid() {
        let root = "module(name = \"consumer\")\ninclude(\"//deps:dx.MODULE.bazel\")\n";
        let read = |_: &str| {
            Ok(
                "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"a//b\")\n"
                    .to_owned(),
            )
        };
        assert_eq!(rules_dx_repo_with(root, &read), "rules_dx");
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
    fn renamed_repo_settings_are_spelled_and_guarded_with_that_repo() {
        let plan = |user: &[String]| bazel_argv_in("my_dx", "build", user, &[], &[], &[], &[]);
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
                matches!(err, ForwardError::ConflictingOption { .. }),
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
