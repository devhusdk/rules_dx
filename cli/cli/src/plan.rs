pub mod generate;
pub mod managed;
pub mod quality;
pub mod registry;
pub mod run_deploy;
pub mod run_temp;
pub mod workflow;

use dx_process::{ForwardError, ProtectedFlag};

pub use generate::{
    generate_package_scope, generate_scope_elements, generate_scope_json, generate_traversal_dirs,
    plan_generate, GenerateScopeElement, GENERATE_CHECK_TARGET, GENERATE_ENV_INTENDED,
    GENERATE_ENV_MODE, GENERATE_ENV_SCOPE, GENERATE_TARGET,
};
pub use managed::{plan_managed, plan_managed_with_roots};
pub(crate) use quality::workflow_scope_labels;
pub use quality::{plan_build, protected_flags, required_options};
pub use registry::{spec, CommandSpec};
pub use run_deploy::{
    plan_deploy_build, plan_deploy_run, plan_run, plan_run_build, plan_run_targets, shell_join,
    shell_quote,
};
pub use run_temp::{
    bep_path, clean_for_stop, create_run_temp_dir, intended_path, remove_on_stop, results_path,
    run_nonce,
};
pub use workflow::{plan_workflow, workflow_options, workflow_protected, WorkflowVerb};

pub const VALIDATE_FLAG: &str = "--@rules_dx//config:validate=false";

pub const CLIPPY_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:clippy_output_diagnostics=true";

pub const RUSTC_DIAGNOSTICS_FLAG: &str = "--@rules_dx//config:rustc_output_diagnostics=true";

pub const BAZEL_SETTINGS: [&str; 3] = [
    VALIDATE_FLAG,
    CLIPPY_DIAGNOSTICS_FLAG,
    RUSTC_DIAGNOSTICS_FLAG,
];

const UPSTREAM_SETTINGS: [&str; 2] = [
    "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
    "--@rules_rust//rust/settings:rustc_output_diagnostics=true",
];

/// Returns the settings every build-like Bazel command shares.
pub fn shared_settings(bazel_command: &str) -> &'static [&'static str] {
    match bazel_command {
        "build" | "test" | "coverage" | "run" | "cquery" | "aquery" => &BAZEL_SETTINGS,
        _ => &[],
    }
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
    let mut required: Vec<String> = settings.iter().map(ToString::to_string).collect();
    required.extend(required_options.iter().cloned());
    let mut guards = protected.to_vec();
    let upstream: &[&str] = if settings.is_empty() {
        &[]
    } else {
        &UPSTREAM_SETTINGS
    };
    for setting in settings.iter().chain(upstream) {
        guards.push(ProtectedFlag {
            name: quality::setting_name(setting)?,
            required: Some((*setting).to_owned()),
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

pub const KEEP_GOING_FLAG: &str = "--keep_going";

pub const SKIP_INCOMPATIBLE_FLAG: &str = "--skip_incompatible_explicit_targets";

pub const DOWNLOAD_ALL_FLAG: &str = "--remote_download_outputs=all";

/// Guards that keep user flags from changing the forced download mode.
pub(crate) fn download_guards() -> Vec<ProtectedFlag> {
    vec![
        ProtectedFlag {
            name: "remote_download_outputs".to_owned(),
            required: Some(DOWNLOAD_ALL_FLAG.to_owned()),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "remote_download_minimal".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "remote_download_toplevel".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ]
}

pub const BEP_FLAG_NAME: &str = "build_event_json_file";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    pub argv: Vec<String>,
    pub summary: String,
}
