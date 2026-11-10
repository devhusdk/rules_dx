use super::*;

/// Runs the luarocks_lock target once, so the named rocks (all of them for a full update) move to their newest release.
fn luarocks_plan(
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
        .iter()
        .find(|lock| lock.ends_with("luarocks.lock"))
        .ok_or_else(|| unsupported("the luarocks.lock tag names no lock"))?;
    let dir = lock.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let Some(target) = dependency_sets::target_label_in(workspace, dir, "luarocks_lock") else {
        return Err(unsupported(
            "add a luarocks_lock next to luarocks.lock; it rewrites luarocks.lock and luarocks.digests.json from luarocks.org",
        ));
    };
    let names = match request {
        Selection::Packages(packages) => packages.clone(),
        Selection::Full => {
            let text = std::fs::read_to_string(workspace.join(lock))
                .map_err(|_| unsupported("luarocks.lock cannot be read"))?;
            luarocks_names(&text)
        }
    };
    if names.is_empty() {
        return Ok(BackendPlan::Noop);
    }
    let mut tail = vec![target.as_str(), "--"];
    tail.extend(names.iter().map(String::as_str));
    Ok(BackendPlan::Run {
        argv: bazel(head, "run", &tail),
        env: Vec::new(),
    })
}

/// The rock names pinned in a luarocks.lock, without `lua`.
fn luarocks_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.trim().trim_end_matches(',');
            if !(value.starts_with('"') && value.ends_with('"') && value.len() > 1) {
                return None;
            }
            let key = key.trim().trim_start_matches("[\"").trim_end_matches("\"]");
            (!key.is_empty() && key != "lua" && !key.contains(char::is_whitespace))
                .then(|| key.to_owned())
        })
        .collect()
}

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    luarocks_plan(workspace, set, request, offline, frozen, head)
}
