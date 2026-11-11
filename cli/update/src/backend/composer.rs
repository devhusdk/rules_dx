use super::*;

const RESOLVER: &str = "@rules_dx//php/private/tools/lock";

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
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
    let dir = workspace.join(&set.dir);
    let dir = dir.to_string_lossy();
    let mut args = vec![RESOLVER, "--", "--dir", &dir];
    if let Selection::Packages(packages) = request {
        for package in packages {
            args.extend(["--upgrade", package]);
        }
    }
    Ok(BackendPlan::Run {
        argv: bazel(head, "run", &args),
        env: Vec::new(),
    })
}

pub(super) fn check(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    _offline: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => Ok(CheckPlan::Run {
            argv: composer_argv(
                workspace,
                set,
                head,
                "@rules_dx//php/private/tools:composer",
                &["validate", "--check-lock", "--no-check-publish"],
            )?,
            env: composer_env(),
        }),
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "composer validates the whole lock; select the set without a package",
        }),
    }
}
