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
        Selection::Full => {
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
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            Ok(BackendPlan::Run {
                argv: configured_fetch_argv(head, &visible, false),
                env: cargo_env("workspace", repository),
            })
        }
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
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            let [package] = packages.as_slice() else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: CARGO_ONE_PACKAGE,
                });
            };
            if cargo_reserved_repin(package) {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: CARGO_RESERVED,
                });
            }
            Ok(BackendPlan::Run {
                argv: configured_fetch_argv(head, &visible, false),
                env: cargo_env(package, repository),
            })
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
            let repository = cargo_repository(set)?;
            let visible = dependency_sets::cargo_fetch_repository(workspace, set)
                .unwrap_or_else(|| repository.to_owned());
            Ok(CheckPlan::Run {
                argv: configured_fetch_argv(head, &visible, offline),
                env: cargo_env("false", repository),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: CARGO_CHECK_SELECTIVE,
        }),
    }
}
