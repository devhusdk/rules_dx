use std::path::Path;

use super::common::*;
use super::managed_codegen::{collect_managed_codegen, empty_generated_id, stage_codegen_side};
use super::managed_env::{collect_managed_env, empty_env_id, stage_env_side};
use crate::args::Command;

pub(crate) fn map_commit_error(error: dx_setup::CommitError) -> (String, String) {
    match error {
        dx_setup::CommitError::NoCapability => (
            CODE_MANAGED_NO_CAPABILITY.to_owned(),
            "selected scope provides neither environment nor codegen capability".to_owned(),
        ),
        other => (CODE_MANAGED_COMMIT_FAILED.to_owned(), other.to_string()),
    }
}

fn lease_staged(
    workspace: &Path,
    kind: dx_clean::GenerationKind,
    id: &dx_setup::GenerationId,
) -> Result<dx_clean::GenerationLease, (String, String)> {
    let dx_dir = workspace.join(dx_env::DX_DIR_NAME);
    dx_clean::acquire_shared_lease(&dx_dir, kind, id.as_str(), dx_clean::LEASE_TIMEOUT).map_err(
        |error| {
            (
                CODE_MANAGED_COMMIT_FAILED.to_owned(),
                format!("cannot lease staged generation: {error}"),
            )
        },
    )
}

pub(crate) fn prepare_managed_sides(
    command: Command,
    repository: bool,
    workspace: &Path,
    bep: &Path,
) -> Result<(dx_setup::PreparedSides, Vec<dx_clean::GenerationLease>), (String, String)> {
    let empties = || dx_setup::PreparedSides {
        prepared_environment: None,
        prepared_generated: None,
        empty_environment: empty_env_id(),
        empty_generated: empty_generated_id(),
    };
    match command {
        Command::Codegen => {
            let (_, plan) = collect_managed_codegen(bep, workspace)?;
            let id = dx_setup::GenerationId::from_digest(plan.digest);
            let lease = lease_staged(workspace, dx_clean::GenerationKind::Generated, &id)?;
            let generated = stage_codegen_side(workspace, &plan)?;
            debug_assert_eq!(generated.as_str(), id.as_str());
            Ok((
                dx_setup::PreparedSides {
                    prepared_generated: Some(generated),
                    ..empties()
                },
                vec![lease],
            ))
        }
        Command::Env => {
            let (_, plan) = collect_managed_env(bep, workspace)?;
            let id = dx_setup::GenerationId::from_digest(plan.digest);
            let lease = lease_staged(workspace, dx_clean::GenerationKind::Environment, &id)?;
            let environment = stage_env_side(workspace, &plan)?;
            debug_assert_eq!(environment.as_str(), id.as_str());
            Ok((
                dx_setup::PreparedSides {
                    prepared_environment: Some(environment),
                    ..empties()
                },
                vec![lease],
            ))
        }
        Command::Setup => {
            let (codegen_outputs, codegen_plan) = collect_managed_codegen(bep, workspace)?;
            let (env_outputs, env_plan) = collect_managed_env(bep, workspace)?;
            let mut leases = Vec::new();
            let prepared_generated = if repository || !codegen_outputs.is_empty() {
                let id = dx_setup::GenerationId::from_digest(codegen_plan.digest);
                leases.push(lease_staged(
                    workspace,
                    dx_clean::GenerationKind::Generated,
                    &id,
                )?);
                Some(stage_codegen_side(workspace, &codegen_plan)?)
            } else {
                None
            };
            let prepared_environment = if repository || !env_outputs.is_empty() {
                let id = dx_setup::GenerationId::from_digest(env_plan.digest);
                leases.push(lease_staged(
                    workspace,
                    dx_clean::GenerationKind::Environment,
                    &id,
                )?);
                Some(stage_env_side(workspace, &env_plan)?)
            } else {
                None
            };
            Ok((
                dx_setup::PreparedSides {
                    prepared_environment,
                    prepared_generated,
                    ..empties()
                },
                leases,
            ))
        }
        // LCOV_EXCL_START - reason: unreached command, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        _ => {
            debug_assert!(false, "managed dispatch guards commands");
            Err((
                CODE_INVALID_RESULT.to_owned(),
                "unsupported managed command".to_owned(),
            ))
        } // LCOV_EXCL_STOP - reason: end unreached command, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn managed_commit_errors_map_to_stable_codes() {
        let (code, message) = map_commit_error(dx_setup::CommitError::NoCapability);
        assert_eq!(code, CODE_MANAGED_NO_CAPABILITY);
        assert!(
            message.contains("neither environment nor codegen"),
            "{message}"
        );
        let (code, _) = map_commit_error(dx_setup::CommitError::WorkspaceRoot {
            path: PathBuf::from("missing"),
        });
        assert_eq!(code, CODE_MANAGED_COMMIT_FAILED);
    }
}
