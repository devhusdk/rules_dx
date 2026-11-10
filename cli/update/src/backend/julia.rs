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
            let Some(resolve) =
                dependency_sets::julia_target_label(workspace, set, "julia_pkg_resolve")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a julia_pkg_resolve next to Manifest.bazel.json; it keeps Manifest.bazel.json in step with Manifest.toml",
                });
            };
            let dir = set_dir(workspace, set)?;
            let mut update = vec![
                "@rules_dx//julia/private/tools:pkg_update",
                "--",
                dir.as_str(),
            ];
            if let Selection::Packages(packages) = request {
                update.extend(packages.iter().map(String::as_str));
            }
            Ok(BackendPlan::Sequence(vec![
                Run {
                    argv: bazel(head, "run", &update),
                    env: julia_env(),
                },
                Run {
                    argv: bazel(head, "run", &[resolve.as_str()]),
                    env: julia_env(),
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
            let Some(test) =
                dependency_sets::julia_target_label(workspace, set, "julia_pkg_resolve_test")
            else {
                return Err(BackendError::Unsupported {
                    set: set.name.clone(),
                    reason: "add a julia_pkg_resolve_test next to Manifest.bazel.json; dx checks the set by running it",
                });
            };
            Ok(CheckPlan::Run {
                argv: bazel(head, "test", &[test.as_str()]),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "julia_pkg_resolve_test checks the whole Manifest.toml; select the set without a package",
        }),
    }
}
