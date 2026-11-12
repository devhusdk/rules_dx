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
        rehash::Rule {
            rule: "fortran_fpm_lock",
            reason: "add a fortran_fpm_lock next to fpm.lock; it rewrites the sha256 pins from the archives",
        },
        workspace,
        set,
        request,
        offline,
        frozen,
        head,
    )
}
