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
        request => {
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
            let mut command = vec!["pub", "upgrade"];
            if let Selection::Packages(packages) = request {
                command.extend(packages.iter().map(String::as_str));
            }
            Ok(BackendPlan::Run {
                argv: pub_argv(workspace, set, head, &command)?,
                env: pub_env(),
            })
        }
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
            let mut command = vec!["pub", "get", "--enforce-lockfile"];
            if offline {
                command.push("--offline");
            }
            Ok(CheckPlan::Run {
                argv: pub_argv(workspace, set, head, &command)?,
                env: pub_env(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "dart pub get checks the whole pubspec.lock; select the set without a package",
        }),
    }
}
