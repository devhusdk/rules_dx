#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};

pub const REPOSITORY_PATTERN: &str = "//...";

pub const PATTERN_FILE_FLAG: &str = "--target_pattern_file";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RootStrategy {
    RecursivePattern,
    QueryPatternFile,
    MonolithicAggregate,
    PackageShards,
}

impl RootStrategy {
    pub fn name(&self) -> &'static str {
        match self {
            RootStrategy::RecursivePattern => "recursive-pattern",
            RootStrategy::QueryPatternFile => "query-pattern-file",
            RootStrategy::MonolithicAggregate => "monolithic-aggregate",
            RootStrategy::PackageShards => "package-shards",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRootPlan {
    pub strategy: RootStrategy,
    pub roots: Vec<String>,
    pub pattern_file: Option<PathBuf>,
}

impl RepositoryRootPlan {
    pub fn baseline() -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::RecursivePattern,
            roots: vec![REPOSITORY_PATTERN.to_owned()],
            pattern_file: None,
        }
    }

    pub fn query_pattern_file(path: &Path) -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::QueryPatternFile,
            roots: Vec::new(),
            pattern_file: Some(path.to_owned()),
        }
    }

    pub fn monolithic_aggregate(label: &str) -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::MonolithicAggregate,
            roots: vec![label.to_owned()],
            pattern_file: None,
        }
    }

    pub fn package_shards(labels: &[String]) -> RepositoryRootPlan {
        RepositoryRootPlan {
            strategy: RootStrategy::PackageShards,
            roots: labels.to_vec(),
            pattern_file: None,
        }
    }

    pub fn pattern_file_arg(&self) -> Option<String> {
        self.pattern_file
            .as_ref()
            .map(|path| format!("{}={}", PATTERN_FILE_FLAG, path.display()))
    }
}

pub fn repository_plan() -> RepositoryRootPlan {
    RepositoryRootPlan::baseline()
}

pub fn invocation_targets(plan: &RepositoryRootPlan, canonical: &str) -> Vec<String> {
    invocation_targets_union(plan, &[canonical])
}

pub fn invocation_targets_union(plan: &RepositoryRootPlan, canonicals: &[&str]) -> Vec<String> {
    if plan.pattern_file.is_some() {
        return Vec::new();
    }
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
    if let Some(pattern_arg) = plan.pattern_file_arg() {
        argv.push(pattern_arg);
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
        assert_eq!(plan.pattern_file, None);
        assert_eq!(plan.pattern_file_arg(), None);
    }

    #[test]
    fn query_file_plan_defers_roots_to_flag_file() {
        let plan = RepositoryRootPlan::query_pattern_file(Path::new("/tmp/roots.txt"));
        assert_eq!(plan.strategy, RootStrategy::QueryPatternFile);
        assert!(plan.roots.is_empty());
        assert_eq!(plan.pattern_file, Some(PathBuf::from("/tmp/roots.txt")));
        assert_eq!(
            plan.pattern_file_arg(),
            Some(format!("{PATTERN_FILE_FLAG}=/tmp/roots.txt"))
        );
    }

    #[test]
    fn aggregate_plans_carry_their_labels() {
        let single = RepositoryRootPlan::monolithic_aggregate("//dx:codegen_roots");
        assert_eq!(single.strategy, RootStrategy::MonolithicAggregate);
        assert_eq!(single.roots, vec![label("//dx:codegen_roots")]);
        assert_eq!(single.pattern_file_arg(), None);

        let shards = RepositoryRootPlan::package_shards(&labels(&["//a:roots", "//b:roots"]));
        assert_eq!(shards.strategy, RootStrategy::PackageShards);
        assert_eq!(shards.roots, labels(&["//a:roots", "//b:roots"]));
        assert_eq!(shards.pattern_file_arg(), None);
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
    fn aggregate_plans_pass_their_roots_through() {
        let single = RepositoryRootPlan::monolithic_aggregate("//dx:codegen_roots");
        assert_eq!(
            invocation_targets(&single, "//dx:codegen"),
            vec![label("//dx:codegen_roots")]
        );
        let shards = RepositoryRootPlan::package_shards(&labels(&["//a:roots", "//b:roots"]));
        assert_eq!(
            invocation_targets(&shards, "//dx:codegen"),
            labels(&["//a:roots", "//b:roots"])
        );
    }

    #[test]
    fn query_file_plan_carries_no_command_line_patterns() {
        let plan = RepositoryRootPlan::query_pattern_file(Path::new("/tmp/roots.txt"));
        assert!(invocation_targets(&plan, "//dx:codegen").is_empty());
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
                "--aspects=//generation:codegen.bzl%dx_codegen_plan_aspect".to_owned(),
                "--output_groups=dx_codegen_plans".to_owned(),
                format!("{PATTERN_FILE_FLAG}=/tmp/roots.txt"),
            ]
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
