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
            let Some(resolve) =
                dependency_sets::julia_target_label(workspace, set, "haskell_deps_resolve")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a haskell_deps_resolve next to cabal.project; it keeps cabal.project.freeze and haskell_deps.json in step",
                });
            };
            Ok(BackendPlan::Run {
                argv: bazel(head, "run", &[resolve.as_str()]),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "haskell_deps_resolve resolves the whole cabal project again; select the set without a package",
        }),
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
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            let Some(test) =
                dependency_sets::julia_target_label(workspace, set, "haskell_deps_resolve_test")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a haskell_deps_resolve_test next to cabal.project; dx checks the set by running it",
                });
            };
            Ok(CheckPlan::Run {
                argv: bazel(head, "test", &[test.as_str()]),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "haskell_deps_resolve_test checks the whole cabal project; select the set without a package",
        }),
    }
}
