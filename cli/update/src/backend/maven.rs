use super::*;

pub(super) fn plan(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<BackendPlan, BackendError> {
    match request {
        Selection::Full => {
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
            Ok(BackendPlan::Run {
                argv: maven_pin_argv(head, maven_repository(set)?),
                env: maven_env(true),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: MAVEN_ONE_PACKAGE,
        }),
    }
}

pub(super) fn check(
    _workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    head: &dx_process::BazelHead,
    _closure: &UvClosure,
) -> Result<CheckPlan, BackendError> {
    match request {
        Selection::Full => {
            let repository = maven_repository(set)?;
            Ok(CheckPlan::Run {
                argv: configured_fetch_argv(head, repository, offline),
                env: maven_env(false),
            })
        }
        Selection::Packages(_) => Err(BackendError::Unsupported {
            set: set.name.clone(),
            reason: MAVEN_ONE_PACKAGE,
        }),
    }
}

pub(super) fn host_env_refusal(
    set: &ResolvedSet,
    check: bool,
    lookup: &dyn Fn(&str) -> bool,
) -> Option<BackendError> {
    let reason = if check && MAVEN_REPIN_ENV.iter().any(|key| lookup(key)) {
        "unset REPIN and RULES_JVM_EXTERNAL_REPIN: any value stops rules_jvm_external from failing a stale lock"
    } else if MAVEN_COURSIER_ENV.iter().any(|key| lookup(key)) {
        "unset COURSIER_MIRRORS, COURSIER_REPOSITORIES, COURSIER_CREDENTIALS, COURSIER_CONFIG_DIR and SCALA_CLI_CONFIG: they redirect where maven artifacts resolve from"
    } else {
        return None;
    };
    Some(BackendError::Unsupported {
        set: set.name.clone(),
        reason,
    })
}
