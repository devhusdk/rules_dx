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
        _ if is_rebar(set) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "rebar.lock refresh: #9884",
        }),
        request => {
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let packages = match request {
                Selection::Full => vec!["--all".to_owned()],
                Selection::Packages(packages) => packages.clone(),
            };
            Ok(BackendPlan::Staged(hex_staged(
                set,
                head,
                offline,
                "deps.update",
                &packages,
            )?))
        }
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
        // Nothing refreshes a rebar.lock yet, so its check has nothing to run.
        Selection::Full if is_rebar(set) => Ok(CheckPlan::Pinned),
        Selection::Packages(_) if is_rebar(set) => Err(pinned::selective(set)),
        Selection::Full => Ok(CheckPlan::Staged(hex_staged(
            set,
            head,
            offline,
            "deps.get",
            &[],
        )?)),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "mix deps.get checks the whole mix.lock; select the set without a package",
        }),
    }
}
