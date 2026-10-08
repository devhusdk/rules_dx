use std::collections::BTreeSet;

use super::outcome::{ReportedStatus, UpdateReport};
use super::sets::SetId;

pub const RECOVERY_CODE: &str = "update_recovery";

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
            ReportedStatus::Failed | ReportedStatus::Blocked
        ) {
            sets.insert(outcome.set.clone());
        }
    }
    sets.into_iter().collect()
}

pub fn restore_paths(report: &UpdateReport) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for outcome in &report.outcomes {
        if outcome.status != ReportedStatus::Updated {
            continue;
        }
        if let Some(set) = SetId::parse(outcome.set.as_str()) {
            for path in set.locks() {
                paths.insert((*path).to_owned());
            }
        }
    }
    paths.into_iter().collect()
}

pub fn retry_command(retry: &[String]) -> String {
    if retry.is_empty() {
        return "dx update --apply".to_owned();
    }
    let mut command = String::from("dx update --apply");
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

pub fn plan(report: &UpdateReport) -> Option<RecoveryPlan> {
    if !report.overall_failure {
        return None;
    }
    let retry = retry_sets(report);
    let restore = restore_paths(report);
    let succeeded = report
        .outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                ReportedStatus::Updated | ReportedStatus::Current | ReportedStatus::ManualPinned
            )
        })
        .count();
    let failed = report
        .outcomes
        .iter()
        .filter(|outcome| outcome.status.fails_run())
        .count();
    let retry_command = retry_command(&retry);
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
    use super::*;

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
        assert_eq!(plan(&clean), None);
    }

    #[test]
    fn failed_report_plans_retry_plus_restore() {
        let plan = plan(&report()).expect("failed report plans recovery");
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
        let plan = plan(&report).expect("recovery");
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
        assert_eq!(retry_command(&[]), "dx update --apply");
        assert_eq!(
            retry_command(&["go".to_owned()]),
            "dx update --apply go".to_owned()
        );
    }
}
