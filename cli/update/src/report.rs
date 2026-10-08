use super::outcome::UpdateReport;

pub const EXIT_SUCCESS: i32 = 0;

pub const EXIT_FAILURE: i32 = 1;

pub fn exit_code(report: &UpdateReport) -> i32 {
    if report.overall_failure {
        EXIT_FAILURE
    } else {
        EXIT_SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::super::outcome::{ReportedOutcome, ReportedStatus};
    use super::*;

    fn report(outcomes: &[(&str, ReportedStatus)], overall_failure: bool) -> UpdateReport {
        UpdateReport {
            outcomes: outcomes
                .iter()
                .map(|(set, status)| ReportedOutcome {
                    set: (*set).to_owned(),
                    status: status.clone(),
                })
                .collect(),
            overall_failure,
        }
    }

    #[test]
    fn clean_run_exits_zero() {
        let clean = report(&[("cargo-lock", ReportedStatus::Updated)], false);
        assert_eq!(exit_code(&clean), EXIT_SUCCESS);
        assert_eq!(exit_code(&clean), 0);
    }

    #[test]
    fn empty_selection_exits_zero() {
        assert_eq!(exit_code(&report(&[], false)), 0);
    }

    #[test]
    fn any_failure_exits_one() {
        let failed = report(
            &[
                ("cargo-lock", ReportedStatus::Updated),
                ("npm-root", ReportedStatus::Failed),
            ],
            true,
        );
        assert_eq!(exit_code(&failed), EXIT_FAILURE);
        assert_eq!(exit_code(&failed), 1);
    }

    #[test]
    fn failure_with_blocked_dependents_exits_one() {
        let failed = report(
            &[
                ("base-set", ReportedStatus::Failed),
                ("app-set", ReportedStatus::Blocked),
            ],
            true,
        );
        assert_eq!(exit_code(&failed), 1);
    }

    #[test]
    fn verdict_follows_overall_flag_not_blocked_presence() {
        let blocked_only = report(&[("app-set", ReportedStatus::Blocked)], false);
        assert_eq!(exit_code(&blocked_only), 0);
    }

    #[test]
    fn unsupported_fails_while_unavailable_does_not() {
        let unsupported = report(&[("cargo", ReportedStatus::Unsupported)], true);
        assert_eq!(exit_code(&unsupported), 1);
        let unavailable = report(
            &[
                ("cargo", ReportedStatus::Unavailable),
                ("uv", ReportedStatus::Current),
            ],
            false,
        );
        assert_eq!(exit_code(&unavailable), 0);
    }
}
