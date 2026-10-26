use std::collections::{BTreeMap, BTreeSet};

use super::outcome::{ReportedStatus, UpdateReport};

pub const RECOVERY_CODE: &str = "update_recovery";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryMode {
    Apply,
    Check,
}

impl RetryMode {
    fn prefix(self) -> &'static str {
        match self {
            RetryMode::Apply => "dx update --apply",
            RetryMode::Check => "dx update",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPlan {
    pub retry_sets: Vec<String>,
    pub restore_paths: Vec<String>,
    pub retry_command: String,
    pub restore_command: Option<String>,
    pub message: String,
}

pub fn retry_sets(report: &UpdateReport) -> Vec<String> {
    let mut sets = BTreeSet::new();
    for outcome in &report.outcomes {
        if matches!(
            outcome.status,
            ReportedStatus::Failed | ReportedStatus::Unsupported | ReportedStatus::Blocked
        ) {
            sets.insert(outcome.set.clone());
        }
    }
    sets.into_iter().collect()
}

/// Lock paths to restore for every Updated outcome, looked up by set name in
/// `locks`; sets missing from the map contribute nothing.
pub fn restore_paths(report: &UpdateReport, locks: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for outcome in &report.outcomes {
        if outcome.status != ReportedStatus::Updated {
            continue;
        }
        if let Some(set_locks) = locks.get(&outcome.set) {
            paths.extend(set_locks.iter().cloned());
        }
    }
    paths.into_iter().collect()
}

pub fn retry_command(retry: &[String], mode: RetryMode) -> String {
    let mut command = String::from(mode.prefix());
    for set in retry {
        command.push(' ');
        command.push_str(set);
    }
    command
}

pub fn restore_command(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let mut command = String::from("git checkout --");
    for path in paths {
        command.push(' ');
        command.push_str(path);
    }
    Some(command)
}

pub fn recovery_message(plan: &RecoveryPlan, succeeded: usize, failed: usize) -> String {
    let mut message = format!(
        "recovery: rerun `{}` for {} not-updated set(s) ({} succeeded, {} failed); retry is idempotent",
        plan.retry_command,
        plan.retry_sets.len(),
        succeeded,
        failed,
    );
    match &plan.restore_command {
        Some(restore) => {
            message.push_str(&format!("; to discard kept successes run `{restore}`"));
        }
        None => {
            message.push_str("; nothing to roll back");
        }
    }
    message
}

pub fn plan(
    report: &UpdateReport,
    mode: RetryMode,
    locks: &BTreeMap<String, Vec<String>>,
) -> Option<RecoveryPlan> {
    if !report.overall_failure {
        return None;
    }
    let retry = retry_sets(report);
    let restore = restore_paths(report, locks);
    let succeeded = report
        .outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                ReportedStatus::Updated | ReportedStatus::Current | ReportedStatus::Pinned
            )
        })
        .count();
    let failed = report
        .outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                ReportedStatus::Unsupported | ReportedStatus::Failed
            )
        })
        .count();
    let retry_command = retry_command(&retry, mode);
    let restore_command = restore_command(&restore);
    let message = recovery_message(
        &RecoveryPlan {
            retry_sets: retry.clone(),
            restore_paths: restore.clone(),
            retry_command: retry_command.clone(),
            restore_command: restore_command.clone(),
            message: String::new(),
        },
        succeeded,
        failed,
    );
    Some(RecoveryPlan {
        retry_sets: retry,
        restore_paths: restore,
        retry_command,
        restore_command,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::super::outcome::{ReportedOutcome, ReportedStatus, UpdateReport};
    use super::super::sets::SetId;
    use super::*;

    fn builtin_locks() -> BTreeMap<String, Vec<String>> {
        SetId::ALL
            .iter()
            .map(|set| {
                (
                    set.name().to_owned(),
                    set.locks().iter().map(|path| (*path).to_owned()).collect(),
                )
            })
            .collect()
    }

    fn report() -> UpdateReport {
        UpdateReport {
            outcomes: vec![
                ReportedOutcome {
                    set: "cargo".to_owned(),
                    status: ReportedStatus::Updated,
                },
                ReportedOutcome {
                    set: "maven".to_owned(),
                    status: ReportedStatus::Failed,
                },
                ReportedOutcome {
                    set: "npm".to_owned(),
                    status: ReportedStatus::Updated,
                },
            ],
            overall_failure: true,
        }
    }

    #[test]
    fn success_needs_no_recovery() {
        let clean = UpdateReport {
            outcomes: vec![ReportedOutcome {
                set: "cargo".to_owned(),
                status: ReportedStatus::Updated,
            }],
            overall_failure: false,
        };
        assert_eq!(plan(&clean, RetryMode::Apply, &builtin_locks()), None);
    }

    #[test]
    fn failed_report_plans_retry_plus_restore() {
        let plan = plan(&report(), RetryMode::Apply, &builtin_locks())
            .expect("failed report plans recovery");
        assert_eq!(plan.retry_sets, vec!["maven".to_owned()]);
        assert_eq!(plan.retry_command, "dx update --apply maven");
        assert!(plan.restore_paths.contains(&"pnpm-lock.yaml".to_owned()));
        assert!(plan
            .restore_paths
            .contains(&"rust/tests/fixtures/hello/Cargo.lock".to_owned()));
        let restore = plan.restore_command.expect("kept successes restore");
        assert!(restore.starts_with("git checkout -- "));
        assert!(restore.contains("pnpm-lock.yaml"));
        assert!(plan.message.contains("dx update --apply maven"));
        assert!(plan.message.contains("idempotent"));
    }

    #[test]
    fn retry_is_sorted_and_restore_is_deduped() {
        let report = UpdateReport {
            outcomes: vec![
                ReportedOutcome {
                    set: "nuget".to_owned(),
                    status: ReportedStatus::Blocked,
                },
                ReportedOutcome {
                    set: "maven".to_owned(),
                    status: ReportedStatus::Failed,
                },
                ReportedOutcome {
                    set: "cargo".to_owned(),
                    status: ReportedStatus::Updated,
                },
            ],
            overall_failure: true,
        };
        let plan = plan(&report, RetryMode::Apply, &builtin_locks()).expect("recovery");
        assert_eq!(
            plan.retry_sets,
            vec!["maven".to_owned(), "nuget".to_owned()]
        );
        assert_eq!(plan.retry_command, "dx update --apply maven nuget");
        let mut sorted = plan.restore_paths.clone();
        sorted.sort();
        assert_eq!(plan.restore_paths, sorted);
    }

    #[test]
    fn retry_command_is_idempotent_shape() {
        let go = ["go".to_owned()];
        assert_eq!(retry_command(&[], RetryMode::Apply), "dx update --apply");
        assert_eq!(retry_command(&go, RetryMode::Apply), "dx update --apply go");
        assert_eq!(retry_command(&[], RetryMode::Check), "dx update");
        assert_eq!(retry_command(&go, RetryMode::Check), "dx update go");
    }

    #[test]
    fn check_mode_retry_omits_apply() {
        let plan = plan(&report(), RetryMode::Check, &builtin_locks())
            .expect("failed report plans recovery");
        assert_eq!(plan.retry_command, "dx update maven");
        assert!(plan.message.contains("rerun `dx update maven`"));
        assert!(!plan.message.contains("--apply"));
    }

    #[test]
    fn unsupported_sets_retry_but_pinned_and_current_never_restore() {
        let report = UpdateReport {
            outcomes: vec![
                ReportedOutcome {
                    set: "cargo".to_owned(),
                    status: ReportedStatus::Unsupported,
                },
                ReportedOutcome {
                    set: "go".to_owned(),
                    status: ReportedStatus::Pinned,
                },
                ReportedOutcome {
                    set: "uv".to_owned(),
                    status: ReportedStatus::Current,
                },
            ],
            overall_failure: true,
        };
        let plan =
            plan(&report, RetryMode::Apply, &builtin_locks()).expect("unsupported plans recovery");
        assert_eq!(plan.retry_sets, vec!["cargo".to_owned()]);
        assert_eq!(plan.retry_command, "dx update --apply cargo");
        assert!(plan.restore_paths.is_empty());
        assert!(plan.restore_command.is_none());
        assert!(plan.message.contains("; nothing to roll back"));
        assert!(plan.message.contains("2 succeeded, 1 failed"));
    }

    #[test]
    fn restore_paths_come_from_the_given_locks() {
        let report = UpdateReport {
            outcomes: vec![
                ReportedOutcome {
                    set: "uv".to_owned(),
                    status: ReportedStatus::Updated,
                },
                ReportedOutcome {
                    set: "cargo".to_owned(),
                    status: ReportedStatus::Failed,
                },
            ],
            overall_failure: true,
        };
        let locks = BTreeMap::from([("uv".to_owned(), vec!["apps/frontend/uv.lock".to_owned()])]);
        let plan = plan(&report, RetryMode::Apply, &locks).expect("recovery");
        assert_eq!(
            plan.restore_command.as_deref(),
            Some("git checkout -- apps/frontend/uv.lock")
        );
        assert!(!plan.message.contains("python/tests/fixtures"));
    }

    #[test]
    fn updated_set_without_locks_restores_nothing() {
        let locks = BTreeMap::new();
        let plan = plan(&report(), RetryMode::Apply, &locks).expect("recovery");
        assert!(plan.restore_paths.is_empty());
        assert!(plan.restore_command.is_none());
        assert!(plan.message.contains("nothing to roll back"));
    }
}
