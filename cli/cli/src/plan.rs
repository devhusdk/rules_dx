pub mod generate;
pub mod managed;
pub mod quality;
pub mod registry;
pub mod run_deploy;
pub mod run_temp;
pub mod workflow;

pub use generate::{
    generate_scope_elements, generate_scope_json, generate_traversal_dirs, plan_generate,
    GenerateScopeElement, GENERATE_CHECK_TARGET, GENERATE_ENV_INTENDED, GENERATE_ENV_MODE,
    GENERATE_ENV_SCOPE, GENERATE_TARGET,
};
pub use managed::{plan_bazel, plan_managed, plan_managed_with_roots};
pub(crate) use quality::workflow_scope_labels;
pub use quality::{plan_build, protected_flags, quality_provenance, required_options};
pub use registry::{spec, CommandSpec};
pub use run_deploy::{
    plan_deploy_build, plan_deploy_run, plan_run, plan_run_build, plan_run_targets, shell_join,
};
pub use run_temp::{bep_path, create_run_temp_dir, intended_path, run_nonce};
pub use workflow::{plan_workflow, workflow_options, workflow_protected, WorkflowVerb};

pub const VALIDATE_FLAG: &str = "--@rules_dx//config:validate=false";

pub const CLIPPY_DIAGNOSTICS_FLAG: &str =
    "--@rules_rust//rust/settings:clippy_output_diagnostics=true";

pub const RUSTC_DIAGNOSTICS_FLAG: &str =
    "--@rules_rust//rust/settings:rustc_output_diagnostics=true";

pub const OUTPUT_GROUP: &str = "dx_results";

pub const KEEP_GOING_FLAG: &str = "--keep_going";

pub const DOWNLOAD_ALL_FLAG: &str = "--remote_download_outputs=all";

pub const BEP_FLAG_NAME: &str = "build_event_json_file";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildPlan {
    pub argv: Vec<String>,
    pub summary: String,
}
