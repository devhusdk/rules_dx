use super::AuditFamily;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyStatus {
    Clean,
    Findings,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyOutcome {
    pub family: AuditFamily,
    pub status: FamilyStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditReport {
    pub outcomes: Vec<FamilyOutcome>,
}

pub const EXIT_SUCCESS: i32 = 0;

pub const EXIT_FAILURE: i32 = 1;

impl AuditFamily {
    fn order(self) -> u8 {
        match self {
            AuditFamily::Security => 0,
            AuditFamily::License => 1,
        }
    }
}

impl AuditReport {
    pub fn aggregate(outcomes: Vec<FamilyOutcome>) -> AuditReport {
        let mut sorted = outcomes;
        sorted.sort_by_key(|outcome| outcome.family.order());
        AuditReport { outcomes: sorted }
    }

    pub fn is_clean(&self) -> bool {
        !self.outcomes.is_empty()
            && self
                .outcomes
                .iter()
                .all(|outcome| outcome.status == FamilyStatus::Clean)
    }

    pub fn incomplete(&self) -> Vec<AuditFamily> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status == FamilyStatus::Incomplete)
            .map(|outcome| outcome.family)
            .collect()
    }
}

pub fn exit_code(report: &AuditReport) -> i32 {
    if report.outcomes.is_empty() || report.is_clean() {
        EXIT_SUCCESS
    } else {
        EXIT_FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(family: AuditFamily, status: FamilyStatus) -> FamilyOutcome {
        FamilyOutcome { family, status }
    }

    #[test]
    fn clean_run_exits_zero() {
        let report = AuditReport::aggregate(vec![
            outcome(AuditFamily::Security, FamilyStatus::Clean),
            outcome(AuditFamily::License, FamilyStatus::Clean),
        ]);
        assert!(report.is_clean());
        assert!(report.incomplete().is_empty());
        assert_eq!(exit_code(&report), EXIT_SUCCESS);
        assert_eq!(exit_code(&report), 0);
    }

    #[test]
    fn findings_exit_one() {
        let report = AuditReport::aggregate(vec![
            outcome(AuditFamily::Security, FamilyStatus::Findings),
            outcome(AuditFamily::License, FamilyStatus::Clean),
        ]);
        assert!(!report.is_clean());
        assert_eq!(exit_code(&report), EXIT_FAILURE);
        assert_eq!(exit_code(&report), 1);
    }

    #[test]
    fn incomplete_assessment_exits_one() {
        let report = AuditReport::aggregate(vec![
            outcome(AuditFamily::Security, FamilyStatus::Clean),
            outcome(AuditFamily::License, FamilyStatus::Incomplete),
        ]);
        assert!(!report.is_clean());
        assert_eq!(report.incomplete(), vec![AuditFamily::License]);
        assert_eq!(exit_code(&report), 1);
    }

    #[test]
    fn aggregation_sorts_canonical_security_first() {
        let report = AuditReport::aggregate(vec![
            outcome(AuditFamily::License, FamilyStatus::Clean),
            outcome(AuditFamily::Security, FamilyStatus::Clean),
        ]);
        let order: Vec<AuditFamily> = report
            .outcomes
            .iter()
            .map(|outcome| outcome.family)
            .collect();
        assert_eq!(order, vec![AuditFamily::Security, AuditFamily::License]);
    }

    #[test]
    fn empty_report_exits_zero_without_claiming_clean() {
        let report = AuditReport::aggregate(vec![]);
        assert!(!report.is_clean());
        assert_eq!(exit_code(&report), 0);
    }
}
