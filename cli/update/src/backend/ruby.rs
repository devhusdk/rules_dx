use super::{BackendError, BackendPlan, CheckPlan, Path, ResolvedSet, Selection, UvClosure, bazel};

const RESOLVER: &str = "@rules_dx//ruby/private/tools/lock";

fn resolver_argv(
    workspace: &Path,
    set: &ResolvedSet,
    head: &dx_process::BazelHead,
    tail: &[&str],
) -> Vec<String> {
    let dir = workspace.join(&set.dir);
    let dir = dir.to_string_lossy();
    let mut args = vec![RESOLVER, "--", "--dir", &dir];
    args.extend_from_slice(tail);
    bazel(head, "run", &args)
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
    let tail: Vec<&str> = match request {
        Selection::Full => Vec::new(),
        Selection::Packages(gems) => gems
            .iter()
            .flat_map(|gem| ["--upgrade", gem.as_str()])
            .collect(),
    };
    Ok(BackendPlan::Run {
        argv: resolver_argv(workspace, set, head, &tail),
        env: Vec::new(),
    })
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
                argv: resolver_argv(workspace, set, head, &["--check"]),
                env: Vec::new(),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: "the ruby lock step checks the whole set; select the set without a gem",
        }),
    }
}
