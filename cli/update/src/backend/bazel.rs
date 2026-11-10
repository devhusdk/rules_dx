use super::*;

pub(super) fn plan(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    _offline: bool,
    _frozen: bool,
    _head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => Ok(BackendPlan::Noop),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "bazel module pins change with `dx update <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
    }
}

pub(super) fn check(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    _offline: bool,
    _head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Pinned),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "bazel module pins change with `dx update <set>:<module> <version>`; refresh the lock with `bazel mod deps`",
        }),
    }
}
