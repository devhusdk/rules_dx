use super::*;

pub(super) fn plan(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    _head: &dx_process::BazelHead,
    closure: &UvClosure,
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
            Ok(BackendPlan::Run {
                argv: configured_uv_argv(set, closure, false, false)?,
                env: vec![],
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
    }
}

pub(super) fn check(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    _head: &dx_process::BazelHead,
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Run {
            argv: configured_uv_argv(set, closure, true, offline)?,
            env: vec![],
        }),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "uv repin refreshes the whole uv lock; select the set without a package",
        }),
    }
}

pub(super) fn host_env_refusal(
    set: &ResolvedSet,
    lookup: &dyn Fn(&str) -> bool,
) -> Option<BackendError> {
    UV_HOST_ENV
        .iter()
        .any(|key| lookup(key))
        .then(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: UV_HOST_ENV_REASON,
        })
}
