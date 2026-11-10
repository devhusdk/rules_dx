use super::*;

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<BackendPlan, BackendError> {
    rehash::plan(
        "fpm_lock",
        "add an fpm_lock next to fpm.lock; it rewrites the sha256 pins from the archives",
        workspace,
        set,
        request,
        offline,
        frozen,
        head,
    )
}
