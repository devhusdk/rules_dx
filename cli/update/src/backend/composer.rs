use super::*;

const RESOLVER: &str = "@rules_dx//php:lock";

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    _request: &Selection,
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
    Ok(BackendPlan::Run {
        argv: bazel(
            head,
            "run",
            &[RESOLVER, "--", "--dir", &dir.to_string_lossy()],
        ),
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
            reason: "composer validates the whole composer.lock; select the set without a package",
        }),
    }
}
