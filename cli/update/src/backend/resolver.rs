use super::{BackendError, BackendPlan, CheckPlan, Ecosystem, Path, ResolvedSet, Selection, bazel};

/// The resolver label of the `setup.hubs` entry of `set`.
pub(super) fn resolver(set: &ResolvedSet) -> Result<&'static str, BackendError> {
    dx_families::hub(set.ecosystem.name())
        .and_then(|hub| hub.resolver.as_deref())
        .ok_or_else(|| BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the setup.hubs entry of this ecosystem names no resolver",
        })
}

fn resolver_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    tail: &[&str],
) -> Result<Vec<String>, BackendError> {
    let resolver = resolver(set)?;
    let dir = workspace.join(&set.dir);
    let dir = dir.to_string_lossy();
    let mut args = vec![resolver, "--", "--dir", &dir];
    args.extend_from_slice(tail);
    Ok(bazel(head, "run", &args))
}

/// The lock step flags that update `request` of `set`.
///
/// The pub step upgrades the named packages, or every package with `--upgrade-all`. The other
/// steps re-resolve the whole set from its manifest.
fn upgrade_flags<'a>(
    set: &ResolvedSet,
    request: &'a Selection,
) -> Result<Vec<&'a str>, BackendError> {
    match (set.ecosystem, request) {
        (Ecosystem::Pub, Selection::Full) => Ok(vec!["--upgrade-all"]),
        (Ecosystem::Pub, Selection::Packages(names)) => Ok(names
            .iter()
            .flat_map(|name| ["--upgrade", name.as_str()])
            .collect()),
        (Ecosystem::Maven, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the maven lock step resolves the versions the version catalog names; edit the catalog or update the whole set",
        }),
        (_, Selection::Full) => Ok(Vec::new()),
        (_, Selection::Packages(_)) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the lock step resolves the whole set; update the whole set",
        }),
    }
}

/// Runs the registry resolver of `set`, which resolves the set afresh and writes its lock.
pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
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
    let flags = upgrade_flags(set, request)?;
    Ok(BackendPlan::Run {
        argv: resolver_argv(workspace, set, head, &flags)?,
        env: Vec::new(),
    })
}

/// Runs the registry resolver of `set` with `--check`, which exits 1 when the lock is stale.
pub(super) fn check(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    head: &dx_process::BazelHead,
) -> Result<CheckPlan, BackendError> {
    if matches!(request, Selection::Packages(_)) {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the lock step checks the whole set; select the set without a package",
        });
    }
    if offline {
        return Err(BackendError::OfflineRequired {
            set: set.name.clone(),
        });
    }
    Ok(CheckPlan::Run {
        argv: resolver_argv(workspace, set, head, &["--check"])?,
        env: Vec::new(),
    })
}
