use super::*;

const RESOLVER: &str = "@rules_dx//perl/private/tools/lock";

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
        Selection::Packages(modules) => {
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
            let mut command = vec!["update"];
            command.extend(modules.iter().map(String::as_str));
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: carton_argv(workspace, set, head, &command)?,
                    env: carton_env(),
                },
                Run {
                    argv: bazel(head, "run", &[RESOLVER, "--", "--dir", &dir.to_string_lossy()]),
                    env: carton_env(),
                },
            ]))
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
                argv: carton_argv(workspace, set, head, &["install", "--deployment"])?,
                env: carton_env(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "carton checks the whole snapshot; select the set without a package",
        }),
    }
}
