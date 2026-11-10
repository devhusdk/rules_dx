use super::*;

/// Runs the ansible_lock target once per collection, so each one moves to its latest Galaxy version.
///
/// A package that is not in the lock yet is added with its dependencies.
fn ansible_plan(
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
    let unsupported = |reason: &'static str| BackendError::Unsupported {
        set: set.name.clone(),
        reason,
    };
    let lock = set
        .locks
        .first()
        .ok_or_else(|| unsupported("the ansible.lock tag names no lock"))?;
    let dir = lock.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let Some(target) = dependency_sets::target_label_in(workspace, dir, "ansible_lock") else {
        return Err(unsupported(
            "add an ansible_lock next to the collections lock; it writes lock.json from Galaxy",
        ));
    };
    let names = match request {
        Selection::Packages(packages) => packages.clone(),
        Selection::Full => {
            let text = std::fs::read_to_string(workspace.join(lock))
                .map_err(|_| unsupported("the collections lock cannot be read"))?;
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|_| unsupported("the collections lock is not valid JSON"))?;
            json.get("collections")
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter_map(|entry| entry.get("name")?.as_str().map(str::to_owned))
                .collect()
        }
    };
    let runs: Vec<Run> = names
        .iter()
        .map(|name| Run {
            argv: bazel(head, "run", &[target.as_str(), "--", name.as_str()]),
            env: Vec::new(),
        })
        .collect();
    if runs.is_empty() {
        return Ok(BackendPlan::Noop);
    }
    Ok(BackendPlan::Sequence(runs))
}

/// A process to run in a directory other than the workspace root.

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    ansible_plan(workspace, set, request, offline, frozen, head)
}
