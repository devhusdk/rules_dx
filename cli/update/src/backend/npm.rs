use super::*;

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    closure: &UvClosure,
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
            let backend = dependency_sets::npm_backend(set);
            let command = match (backend, request) {
                (Some(dependency_sets::NpmBackend::Yarn), Selection::Full) => {
                    strings(&["install", "--mode=update-lockfile"])
                }
                (Some(dependency_sets::NpmBackend::Yarn), Selection::Packages(packages)) => [
                    strings(&["up"]),
                    packages.clone(),
                    strings(&["--mode=update-lockfile"]),
                ]
                .concat(),
                (Some(dependency_sets::NpmBackend::Npm), Selection::Full) => {
                    strings(&["install", "--package-lock-only", "--ignore-scripts"])
                }
                (Some(dependency_sets::NpmBackend::Npm), Selection::Packages(packages)) => [
                    strings(&["update"]),
                    packages.clone(),
                    strings(&["--package-lock-only", "--ignore-scripts"]),
                ]
                .concat(),
                (_, Selection::Full) => strings(&["install", "--lockfile-only"]),
                (_, Selection::Packages(packages)) => [
                    strings(&["update"]),
                    packages.clone(),
                    strings(&["--lockfile-only"]),
                ]
                .concat(),
            };
            let argv = match configured_node_argv(workspace, set, closure, &command)? {
                Some(argv) => argv,
                None => configured_pnpm_argv(workspace, head, set, false, &command)?,
            };
            Ok(BackendPlan::Run {
                argv,
                env: npm_env(),
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
    closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => {
            let backend = dependency_sets::npm_backend(set);
            let mut command = match backend {
                Some(dependency_sets::NpmBackend::Yarn) => {
                    strings(&["install", "--immutable", "--mode=update-lockfile"])
                }
                Some(dependency_sets::NpmBackend::Npm) => {
                    strings(&["ci", "--dry-run", "--ignore-scripts"])
                }
                _ => strings(&["install", "--lockfile-only", "--frozen-lockfile"]),
            };
            if offline && backend != Some(dependency_sets::NpmBackend::Yarn) {
                command.push("--offline".to_owned());
            }
            let argv = match configured_node_argv(workspace, set, closure, &command)? {
                Some(argv) => argv,
                None => configured_pnpm_argv(workspace, head, set, offline, &command)?,
            };
            Ok(CheckPlan::Run {
                argv,
                env: npm_env(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the npm package manager checks the whole lock; select the set without a package",
        }),
    }
}
