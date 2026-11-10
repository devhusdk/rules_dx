use super::*;

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => Ok(BackendPlan::Noop),
        Selection::Packages(packages) => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let Some(manifest) = set.manifests.first().and_then(|m| m.rsplit('/').next()) else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "the set has no .opam file",
                });
            };
            let mut upgrade = vec!["upgrade"];
            upgrade.extend(packages.iter().map(String::as_str));
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: opam_argv(workspace, set, head, &upgrade)?,
                    env: opam_env(),
                },
                Run {
                    argv: opam_argv(workspace, set, head, &["lock", manifest])?,
                    env: opam_env(),
                },
            ]))
        }
    }
}

pub(super) fn check(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => {
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            Ok(CheckPlan::Run {
                argv: opam_argv(
                    workspace,
                    set,
                    head,
                    &[
                        "install",
                        "--locked",
                        "--deps-only",
                        "--dry-run",
                        "--show-actions",
                        ".",
                    ],
                )?,
                env: opam_env(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "opam checks the whole .opam.locked file; select the set without a package",
        }),
    }
}
