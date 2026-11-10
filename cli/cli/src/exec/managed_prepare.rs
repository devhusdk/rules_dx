use std::path::{Path, PathBuf};

use super::common::{
    CODE_INVALID_BEP, CODE_INVALID_RESULT, CODE_MANAGED_COMMIT_FAILED, CODE_UNREADABLE_BEP,
};
use super::managed_codegen::{collect_managed_codegen, empty_generated_id, stage_codegen_side};
use super::managed_env::{collect_managed_env, empty_env_id, stage_env_side};

pub(crate) fn map_commit_error(error: dx_setup::CommitError) -> dx_error::DxError {
    match error {
        dx_setup::CommitError::NoCapability => dx_error::DxError::failure(
            "selected scope provides neither environment nor codegen capability",
        ),
        other => dx_error::DxError::failure(other.to_string()),
    }
}

/// The stable code for a [`prepare_managed_sides`] error.
pub(crate) fn prepare_error_code(error: &dx_error::DxError) -> &'static str {
    let message = error.message();
    if message.starts_with("failed to read build events: ") {
        CODE_UNREADABLE_BEP
    } else if message.starts_with("invalid build events")
        || message.starts_with("invalid BEP config")
    {
        CODE_INVALID_BEP
    } else if message.starts_with("invalid codegen plan") || message.starts_with("invalid env plan")
    {
        CODE_INVALID_RESULT
    } else {
        CODE_MANAGED_COMMIT_FAILED
    }
}

/// The staged sides of one sync, the leases that keep them, and the artifacts the codegen side links.
pub(crate) struct PreparedSync {
    pub(crate) sides: dx_setup::PreparedSides,
    pub(crate) leases: Vec<dx_atomic_fs::lease::SharedLease>,
    pub(crate) artifacts: Vec<PathBuf>,
    pub(crate) sources: Vec<PathBuf>,
}

pub(crate) fn prepare_managed_sides(
    workspace: &Path,
    state_root: &Path,
    bep: &Path,
) -> Result<PreparedSync, dx_error::DxError> {
    let (_, env_plan) = collect_managed_env(bep, workspace)?;
    let (_, codegen_plan) = collect_managed_codegen(bep, workspace)?;
    let (environment, env_lease) = stage_env_side(state_root, &env_plan)?;
    let (generated, codegen_lease) = stage_codegen_side(state_root, workspace, &codegen_plan)?;
    Ok(PreparedSync {
        sides: dx_setup::PreparedSides {
            prepared_environment: Some(environment),
            prepared_generated: Some(generated),
            empty_environment: empty_env_id(),
            empty_generated: empty_generated_id(),
        },
        leases: vec![env_lease, codegen_lease],
        artifacts: codegen_plan
            .projection
            .iter()
            .map(|entry| PathBuf::from(&entry.artifact))
            .collect(),
        sources: codegen_plan.sources.iter().map(PathBuf::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn managed_commit_errors_map_to_failures() {
        let error = map_commit_error(dx_setup::CommitError::NoCapability);
        assert!(
            error.message().contains("neither environment nor codegen"),
            "{error}"
        );
        assert_eq!(error.exit_code(), dx_error::EXIT_FAILURE);
        let error = map_commit_error(dx_setup::CommitError::WorkspaceRoot {
            path: PathBuf::from("missing"),
        });
        assert_eq!(error.exit_code(), dx_error::EXIT_FAILURE);
    }
}
