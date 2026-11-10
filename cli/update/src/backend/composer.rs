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
        Selection::Full => Ok(BackendPlan::Noop),
        Selection::Packages(packages) => {
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
            let mut command = vec!["update"];
            command.extend(packages.iter().map(String::as_str));
            command.extend(["--with-all-dependencies", "--no-install"]);
            let mut runs = vec![Run {
                argv: composer_argv(
                    workspace,
                    set,
                    head,
                    "@rules_dx//php/private/tools:composer",
                    &command,
                )?,
                env: composer_env(),
            }];
            let digests = set
                .locks
                .iter()
                .find(|lock| lock.ends_with(".json"))
                .and_then(|lock| Path::new(lock).file_name())
                .and_then(|name| name.to_str());
            if let Some(digests) = digests {
                runs.push(Run {
                    argv: composer_argv(
                        workspace,
                        set,
                        head,
                        "@rules_dx//php/private/tools/digests:refresh_digests",
                        &["composer.lock", digests],
                    )?,
                    env: composer_env(),
                });
            }
            Ok(BackendPlan::Sequence(runs))
        }
    }
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
