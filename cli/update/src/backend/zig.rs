use super::*;

const RESOLVER: &str = "@rules_dx//zig/private/tools/lock";

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
    if matches!(request, Selection::Packages(_)) {
        return Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the zig lock step resolves every package of the set; update the whole set",
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
