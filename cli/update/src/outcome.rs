use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetStatus {
    Updated,
    Current,
    ManualPinned,
    Unsupported,
    Unavailable,
    Failed,
}

impl SetStatus {
    pub fn fails_run(&self) -> bool {
        matches!(self, SetStatus::Unsupported | SetStatus::Failed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetOutcome {
    pub set: String,
    pub status: SetStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportedStatus {
    Updated,
    Current,
    ManualPinned,
    Unsupported,
    Unavailable,
    Failed,
    Blocked,
}

impl ReportedStatus {
    pub fn fails_run(&self) -> bool {
        matches!(self, ReportedStatus::Unsupported | ReportedStatus::Failed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedOutcome {
    pub set: String,
    pub status: ReportedStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateReport {
    pub outcomes: Vec<ReportedOutcome>,
    pub overall_failure: bool,
}

impl UpdateReport {
    pub fn successes(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome.status,
                    ReportedStatus::Updated
                        | ReportedStatus::Current
                        | ReportedStatus::ManualPinned
                )
            })
            .map(|outcome| outcome.set.clone())
            .collect()
    }

    pub fn failures(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status.fails_run())
            .map(|outcome| outcome.set.clone())
            .collect()
    }

    pub fn blocked(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status == ReportedStatus::Blocked)
            .map(|outcome| outcome.set.clone())
            .collect()
    }

    pub fn unavailable(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.status == ReportedStatus::Unavailable)
            .map(|outcome| outcome.set.clone())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AggregateError {
    #[error("update set {set:?} has no reported result; refusing to guess")]
    MissingResult { set: String },
}

pub fn aggregate(
    selected: &[String],
    results: &[SetOutcome],
    depends_on: &BTreeMap<String, Vec<String>>,
) -> Result<UpdateReport, AggregateError> {
    let mut selected_sets = BTreeSet::new();
    for set in selected {
        selected_sets.insert(set.clone());
    }
    let mut reported: BTreeMap<String, ReportedStatus> = BTreeMap::new();
    for result in results {
        if !selected_sets.contains(&result.set) {
            continue;
        }
        let status = match result.status {
            SetStatus::Updated => ReportedStatus::Updated,
            SetStatus::Current => ReportedStatus::Current,
            SetStatus::ManualPinned => ReportedStatus::ManualPinned,
            SetStatus::Unsupported => ReportedStatus::Unsupported,
            SetStatus::Unavailable => ReportedStatus::Unavailable,
            SetStatus::Failed => ReportedStatus::Failed,
        };
        reported.insert(result.set.clone(), status);
    }
    let failed: BTreeSet<String> = reported
        .iter()
        .filter_map(|(set, status)| {
            if status.fails_run() {
                Some(set.clone())
            } else {
                None
            }
        })
        .collect();
    for set in &selected_sets {
        if reported.contains_key(set) {
            continue;
        }
        if depends_on_failed(set, depends_on, &failed) {
            reported.insert(set.clone(), ReportedStatus::Blocked);
        } else {
            debug_assert!(results_missing(set, results));
            return Err(AggregateError::MissingResult { set: set.clone() });
        }
    }
    let overall_failure = reported.values().any(|status| status.fails_run());
    let mut outcomes: Vec<ReportedOutcome> = reported
        .into_iter()
        .map(|(set, status)| ReportedOutcome { set, status })
        .collect();
    outcomes.sort_by(|left, right| left.set.cmp(&right.set));
    Ok(UpdateReport {
        outcomes,
        overall_failure,
    })
}

pub fn depends_on_failed(
    set: &str,
    depends_on: &BTreeMap<String, Vec<String>>,
    failed: &BTreeSet<String>,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut stack: Vec<String> = depends_on.get(set).cloned().unwrap_or_default();
    while let Some(next) = stack.pop() {
        if !visited.insert(next.clone()) {
            continue;
        }
        if failed.contains(&next) {
            return true;
        }
        if let Some(parents) = depends_on.get(&next) {
            stack.extend(parents.iter().cloned());
        }
    }
    false
}

pub fn results_missing(set: &str, results: &[SetOutcome]) -> bool {
    !results.iter().any(|result| result.set == set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets(words: &[&str]) -> Vec<String> {
        words.iter().copied().map(str::to_owned).collect()
    }

    fn outcomes(pairs: &[(&str, SetStatus)]) -> Vec<SetOutcome> {
        pairs
            .iter()
            .map(|(set, status)| SetOutcome {
                set: (*set).to_owned(),
                status: *status,
            })
            .collect()
    }

    fn empty_deps() -> BTreeMap<String, Vec<String>> {
        BTreeMap::new()
    }

    #[test]
    fn independent_failure_preserves_success_and_fails_overall() {
        let report = aggregate(
            &sets(&["cargo-lock", "npm-root"]),
            &outcomes(&[
                ("cargo-lock", SetStatus::Updated),
                ("npm-root", SetStatus::Failed),
            ]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        assert!(report.overall_failure);
        assert_eq!(report.successes(), vec!["cargo-lock".to_owned()]);
        assert_eq!(report.failures(), vec!["npm-root".to_owned()]);
        assert!(report.blocked().is_empty());
    }

    #[test]
    fn all_success_passes_overall() {
        let report = aggregate(
            &sets(&["cargo-lock", "npm-root"]),
            &outcomes(&[
                ("cargo-lock", SetStatus::Updated),
                ("npm-root", SetStatus::Current),
            ]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        assert!(!report.overall_failure);
        assert!(report.failures().is_empty());
        assert!(report.blocked().is_empty());
    }

    #[test]
    fn current_and_pinned_count_as_success_without_resolver_freshness() {
        let report = aggregate(
            &sets(&["uv", "go"]),
            &outcomes(&[("uv", SetStatus::Current), ("go", SetStatus::ManualPinned)]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        assert!(!report.overall_failure);
        assert_eq!(report.successes(), vec!["go".to_owned(), "uv".to_owned()]);
        assert!(report.failures().is_empty());
    }

    #[test]
    fn unsupported_fails_the_run_like_a_failure() {
        let report = aggregate(
            &sets(&["cargo:anyhow", "npm"]),
            &outcomes(&[
                ("cargo:anyhow", SetStatus::Unsupported),
                ("npm", SetStatus::Updated),
            ]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        assert!(report.overall_failure);
        assert_eq!(report.failures(), vec!["cargo:anyhow".to_owned()]);
        assert_eq!(report.successes(), vec!["npm".to_owned()]);
    }

    #[test]
    fn unavailable_does_not_fail_the_run() {
        let report = aggregate(
            &sets(&["cargo", "uv"]),
            &outcomes(&[
                ("cargo", SetStatus::Unavailable),
                ("uv", SetStatus::Current),
            ]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        assert!(!report.overall_failure);
        assert!(report.failures().is_empty());
        assert_eq!(report.unavailable(), vec!["cargo".to_owned()]);
        assert_eq!(report.successes(), vec!["uv".to_owned()]);
    }

    #[test]
    fn dependent_of_failure_reports_blocked_not_successful() {
        let mut deps = BTreeMap::new();
        deps.insert("app-set".to_owned(), vec!["base-set".to_owned()]);
        let report = aggregate(
            &sets(&["base-set", "app-set"]),
            &outcomes(&[("base-set", SetStatus::Failed)]),
            &deps,
        )
        .expect("dependent blocks");
        assert!(report.overall_failure);
        assert_eq!(report.failures(), vec!["base-set".to_owned()]);
        assert_eq!(report.blocked(), vec!["app-set".to_owned()]);
        assert!(report.successes().is_empty());
    }

    #[test]
    fn transitive_dependent_reports_blocked() {
        let mut deps = BTreeMap::new();
        deps.insert("mid".to_owned(), vec!["base".to_owned()]);
        deps.insert("top".to_owned(), vec!["mid".to_owned()]);
        let report = aggregate(
            &sets(&["base", "mid", "top"]),
            &outcomes(&[("base", SetStatus::Failed)]),
            &deps,
        )
        .expect("transitive dependents block");
        assert_eq!(report.failures(), vec!["base".to_owned()]);
        assert_eq!(report.blocked(), vec!["mid".to_owned(), "top".to_owned()]);
    }

    #[test]
    fn unrelated_set_still_attempts_despite_failure_elsewhere() {
        let mut deps = BTreeMap::new();
        deps.insert("app-set".to_owned(), vec!["base-set".to_owned()]);
        let report = aggregate(
            &sets(&["base-set", "app-set", "other-set"]),
            &outcomes(&[
                ("base-set", SetStatus::Failed),
                ("other-set", SetStatus::Updated),
            ]),
            &deps,
        )
        .expect("independent set attempts");
        assert!(report.overall_failure);
        assert_eq!(report.successes(), vec!["other-set".to_owned()]);
        assert_eq!(report.blocked(), vec!["app-set".to_owned()]);
    }

    #[test]
    fn missing_result_without_failed_dependency_errors() {
        let error = aggregate(&sets(&["cargo-lock"]), &[], &empty_deps())
            .expect_err("unexplained gap must error");
        assert_eq!(
            error,
            AggregateError::MissingResult {
                set: "cargo-lock".to_owned()
            }
        );
    }

    #[test]
    fn unselected_results_are_ignored() {
        let report = aggregate(
            &sets(&["cargo-lock"]),
            &outcomes(&[
                ("cargo-lock", SetStatus::Updated),
                ("elsewhere", SetStatus::Failed),
            ]),
            &empty_deps(),
        )
        .expect("unselected results ignored");
        assert!(!report.overall_failure);
        assert_eq!(report.successes(), vec!["cargo-lock".to_owned()]);
    }

    #[test]
    fn report_order_is_sorted_and_deterministic() {
        let report = aggregate(
            &sets(&["b-set", "a-set"]),
            &outcomes(&[("b-set", SetStatus::Updated), ("a-set", SetStatus::Updated)]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        let order: Vec<&str> = report
            .outcomes
            .iter()
            .map(|outcome| outcome.set.as_str())
            .collect();
        assert_eq!(order, vec!["a-set", "b-set"]);
    }

    #[test]
    fn execution_gaps_parallelism_stays_sequential() {
        let report = aggregate(
            &sets(&["c-set", "a-set", "b-set"]),
            &outcomes(&[
                ("c-set", SetStatus::Updated),
                ("a-set", SetStatus::Updated),
                ("b-set", SetStatus::Updated),
            ]),
            &empty_deps(),
        )
        .expect("complete results aggregate");
        let order: Vec<&str> = report
            .outcomes
            .iter()
            .map(|outcome| outcome.set.as_str())
            .collect();
        assert_eq!(order, vec!["a-set", "b-set", "c-set"]);
        assert!(!report.overall_failure);
    }
}
