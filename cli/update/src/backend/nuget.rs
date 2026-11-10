use super::*;

pub(super) fn plan(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    _frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match request {
        // paket.lock never changes, so a frozen run is a pinned fetch and stays allowed.
        Selection::Full => Ok(BackendPlan::Staged(nuget_staged(set, head, offline)?)),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: NUGET_PINS_EXACT,
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
        Selection::Full => Ok(CheckPlan::Staged(nuget_staged(set, head, offline)?)),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: NUGET_PINS_EXACT,
        }),
    }
}
