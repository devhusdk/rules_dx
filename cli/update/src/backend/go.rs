use super::*;

pub(super) fn plan(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => {
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            Ok(BackendPlan::Staged(go_staged(set, head, offline)?))
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: GO_WHOLE_GRAPH,
        }),
    }
}

pub(super) fn check(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Staged(go_staged(set, head, offline)?)),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: GO_WHOLE_GRAPH,
        }),
    }
}

pub(super) fn host_env_refusal(
    set: &ResolvedSet,
    lookup: &dyn Fn(&str) -> bool,
) -> Option<BackendError> {
    GO_HOST_ENV
        .iter()
        .any(|key| lookup(key))
        .then(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: GO_HOST_ENV_REASON,
        })
}
