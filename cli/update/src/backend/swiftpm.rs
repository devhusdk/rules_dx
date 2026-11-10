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
        selection => {
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
            let mut command = vec!["package".to_owned(), "update".to_owned()];
            if let Selection::Packages(packages) = selection {
                command.extend(packages.iter().map(|name| swift_identity(name)));
            }
            Ok(BackendPlan::Run {
                argv: swift_argv(workspace, set, head, true, &command)?,
                env: Vec::new(),
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
            if offline {
                return Err(BackendError::OfflineRequired {
                    set: set.name.clone(),
                });
            }
            Ok(CheckPlan::Run {
                argv: swift_argv(
                    workspace,
                    set,
                    head,
                    false,
                    &strings(&["package", "resolve"]),
                )?,
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "swift package resolve checks the whole Package.resolved; select the set without a package",
        }),
    }
}
