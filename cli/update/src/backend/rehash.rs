use super::*;

/// Runs the zig_digests or fpm_lock target next to the set, which re-hashes the archives the set pins.
pub(super) fn plan(
    rule: &'static str,
    reason: &'static str,
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
    let Some(target) = dependency_sets::target_label_in(workspace, &set.dir, rule) else {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason,
        });
    };
    let mut tail = vec![target.as_str(), "--"];
    if let Selection::Packages(packages) = request {
        tail.extend(packages.iter().map(String::as_str));
    }
    if tail.len() == 2 {
        tail.pop();
    }
    Ok(BackendPlan::Run {
        argv: bazel(head, "run", &tail),
        env: Vec::new(),
    })
}
