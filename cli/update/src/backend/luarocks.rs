use super::*;

const RESOLVER: &str = "@rules_dx//lua:lock";

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
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
    if matches!(request, Selection::Packages(_)) {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the luarocks lock step resolves every rock of the set; update the whole set",
        });
    }
    let dir = workspace.join(&set.dir);
    Ok(BackendPlan::Run {
        argv: bazel(
            head,
            "run",
            &[RESOLVER, "--", "--dir", &dir.to_string_lossy()],
        ),
        env: Vec::new(),
    })
}
