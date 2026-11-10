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
            let Some(compiler) = dependency_sets::cpan_compiler_label(workspace, set) else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a cpan_compiler next to the cpanfile; it keeps cpanfile.snapshot.lock.json in step with cpanfile.snapshot",
                });
            };
            let mut command = vec!["update"];
            command.extend(modules.iter().map(String::as_str));
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: carton_argv(workspace, set, head, &command)?,
                    env: carton_env(),
                },
                Run {
                    argv: bazel(head, "run", &[&compiler, "--", "--incremental"]),
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
            reason: "carton checks the whole cpanfile.snapshot; select the set without a package",
        }),
    }
}
