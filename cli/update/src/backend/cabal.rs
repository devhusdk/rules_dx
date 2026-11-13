use super::*;

const RESOLVER: &str = "@rules_dx//haskell/private/tools/lock";

fn resolver_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    check: bool,
) -> Vec<String> {
    let dir = workspace.join(&set.dir);
    let dir = dir.to_string_lossy();
    let mut args = vec![RESOLVER, "--", "--dir", &dir];
    if check {
        args.push("--check");
    }
    bazel(head, "run", &args)
}

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
            Ok(BackendPlan::Run {
                argv: resolver_argv(workspace, set, head, false),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the cabal lock step resolves the whole cabal project again; select the set without a package",
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
            Ok(CheckPlan::Run {
                argv: resolver_argv(workspace, set, head, true),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the cabal lock step checks the whole cabal project; select the set without a package",
        }),
    }
}
