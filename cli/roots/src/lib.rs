#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub const REPOSITORY_PATTERN: &str = "//...";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RootStrategy {
    RecursivePattern,
    MonolithicAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRootPlan {
    pub strategy: RootStrategy,
    pub roots: Vec<String>,
}

impl RepositoryRootPlan {
    pub fn baseline() -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::RecursivePattern,
            roots: vec![REPOSITORY_PATTERN.to_owned()],
        }
    }

    pub fn monolithic_aggregate(label: &str) -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::MonolithicAggregate,
            roots: vec![label.to_owned()],
        }
    }
}

pub fn repository_plan() -> RepositoryRootPlan {
    RepositoryRootPlan::baseline()
}

pub fn invocation_targets(plan: &RepositoryRootPlan, canonical: &str) -> Vec<String> {
    invocation_targets_union(plan, &[canonical])
}

pub fn invocation_targets_union(plan: &RepositoryRootPlan, canonicals: &[&str]) -> Vec<String> {
    if plan.strategy == RootStrategy::RecursivePattern
        && plan.roots == vec![REPOSITORY_PATTERN.to_owned()]
    {
        return canonicals
            .iter()
            .map(|canonical| (*canonical).to_owned())
            .collect();
    }
    plan.roots.clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactScopeError {
    MultipleTargets { count: usize },
    TargetPattern { value: String },
    NotTargetLabel { value: String },
}

impl std::fmt::Display for ExactScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExactScopeError::MultipleTargets { count } => {
                write!(f, "expected at most one target, found {count}")
            }
            ExactScopeError::TargetPattern { value } => {
                write!(f, "invalid target {value:?}: patterns never select scope")
            }
            ExactScopeError::NotTargetLabel { value } => {
                write!(f, "invalid target {value:?}: want an exact // or @ label")
            }
        }
    }
}

impl std::error::Error for ExactScopeError {}

pub fn resolve_exact_target(targets: &[String]) -> Result<Option<String>, ExactScopeError> {
    match targets {
        [] => Ok(None),
        [single] => {
            if single.contains("...") || single.contains('*') || single.contains('?') {
                Err(ExactScopeError::TargetPattern {
                    value: single.clone(),
                })
            } else if single.starts_with("//") || single.starts_with('@') {
                Ok(Some(single.clone()))
            } else {
                Err(ExactScopeError::NotTargetLabel {
                    value: single.clone(),
                })
            }
        }
        _ => Err(ExactScopeError::MultipleTargets {
            count: targets.len(),
        }),
    }
}

pub fn build_argv(
    plan: &RepositoryRootPlan,
    canonical: &str,
    aspects: &[String],
    output_groups: &[String],
) -> Vec<String> {
    build_argv_union(plan, &[canonical], aspects, output_groups)
}

pub fn build_argv_union(
    plan: &RepositoryRootPlan,
    canonicals: &[&str],
    aspects: &[String],
    output_groups: &[String],
) -> Vec<String> {
    let mut argv = vec!["build".to_owned()];
    argv.extend(invocation_targets_union(plan, canonicals));
    for aspect in aspects {
        argv.push(format!("--aspects={aspect}"));
    }
    for group in output_groups {
        argv.push(format!("--output_groups={group}"));
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> String {
        value.to_owned()
    }

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| label(value)).collect()
    }

    #[test]
    fn baseline_plan_applies_aspects_to_recursive_pattern() {
        let plan = RepositoryRootPlan::baseline();
        assert_eq!(plan.strategy, RootStrategy::RecursivePattern);
        assert_eq!(plan.roots, vec![REPOSITORY_PATTERN.to_owned()]);
    }

    #[test]
    fn aggregate_plan_carries_its_label() {
        let plan = RepositoryRootPlan::monolithic_aggregate("//dx:codegen_roots");
        assert_eq!(plan.strategy, RootStrategy::MonolithicAggregate);
        assert_eq!(plan.roots, vec![label("//dx:codegen_roots")]);
    }

    #[test]
    fn repository_plan_is_the_baseline() {
        assert_eq!(repository_plan(), RepositoryRootPlan::baseline());
    }

    #[test]
    fn baseline_plan_keeps_the_canonical_selection_identity() {
        let plan = repository_plan();
        assert_eq!(
            invocation_targets(&plan, "//dx:codegen"),
            vec![label("//dx:codegen")]
        );
    }

    #[test]
    fn aggregate_plan_passes_its_root_through() {
        let plan = RepositoryRootPlan::monolithic_aggregate("//dx:codegen_roots");
        assert_eq!(
            invocation_targets(&plan, "//dx:codegen"),
            vec![label("//dx:codegen_roots")]
        );
    }

    #[test]
    fn build_argv_composes_baseline_invocation() {
        let plan = repository_plan();
        let argv = build_argv(
            &plan,
            "//dx:codegen",
            &labels(&["//generation:codegen.bzl%dx_codegen_plan_aspect"]),
            &labels(&["dx_codegen_plans"]),
        );
        assert_eq!(
            argv,
            vec![
                "build".to_owned(),
                "//dx:codegen".to_owned(),
                "--aspects=//generation:codegen.bzl%dx_codegen_plan_aspect".to_owned(),
                "--output_groups=dx_codegen_plans".to_owned(),
            ]
        );
    }
}
