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
        request if is_rebar(set) => {
            if frozen {
                return Err(BackendError::FrozenLocked {
                    set: set.name.clone(),
                });
            }
            let args = match request {
                Selection::Full => vec!["upgrade".to_owned(), "--all".to_owned()],
                Selection::Packages(packages) => {
                    vec!["upgrade".to_owned(), packages.join(",")]
                }
            };
            Ok(BackendPlan::Staged(rebar_staged(
                set, head, offline, &args,
            )?))
        }
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
        Selection::Full if is_rebar(set) => Ok(CheckPlan::Staged(rebar_staged(
            set,
            head,
            offline,
            &["lock".to_owned()],
        )?)),
        Selection::Packages(_) if is_rebar(set) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "rebar3 lock checks the whole rebar.lock; select the set without a package",
        }),
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
