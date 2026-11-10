use super::*;

/// Refuses a package update in a pinned set; dx never rewrites its lock.
pub(super) fn selective(set: &ResolvedSet) -> BackendError {
    BackendError::Unsupported {
        set: set.name.clone(),
        reason: set
            .ecosystem
            .pinned_reason()
            .unwrap_or("dx only pins this set"),
    }
}

pub(super) fn plan(set: &ResolvedSet, request: &Selection) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => Ok(BackendPlan::Noop),
        Selection::Packages(_) => Err(selective(set)),
    }
}

pub(super) fn check(set: &ResolvedSet, request: &Selection) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Pinned),
        Selection::Packages(_) => Err(selective(set)),
    }
}
