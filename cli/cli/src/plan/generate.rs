use dx_process::{build_workflow_argv, describe_scope, ForwardError};

use super::{workflow_scope_labels, BuildPlan};
use crate::resolve::ResolvedScope;

pub const GENERATE_TARGET: &str = "//dx:generate";
pub const GENERATE_CHECK_TARGET: &str = "//dx:generate_check";

pub const GENERATE_ENV_INTENDED: &str = "DX_GENERATE_INTENDED";
pub const GENERATE_ENV_SCOPE: &str = "DX_GENERATE_SCOPE";
pub const GENERATE_ENV_MODE: &str = "DX_GENERATE_MODE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateScopeElement {
    pub element: String,
    pub dirs: Vec<String>,
}

fn generate_traversal_dir(target: &str) -> String {
    if target == "//..." {
        return String::new();
    }
    let Some(rest) = target.strip_prefix("//") else {
        return String::new();
    };
    if let Some(dir) = rest.strip_suffix("/...") {
        return dir.to_owned();
    }
    match rest.split_once(':') {
        Some((package, _)) => package.to_owned(),
        None => rest.to_owned(),
    }
}

pub fn generate_scope_elements(resolved: &ResolvedScope) -> Vec<GenerateScopeElement> {
    let (_, labels) = workflow_scope_labels(resolved);
    labels
        .iter()
        .map(|target| GenerateScopeElement {
            element: target.clone(),
            dirs: vec![generate_traversal_dir(target)],
        })
        .collect()
}

pub fn generate_scope_json(resolved: &ResolvedScope) -> String {
    let items: Vec<serde_json::Value> = generate_scope_elements(resolved)
        .iter()
        .map(|element| {
            serde_json::json!({"element": element.element.clone(), "dirs": element.dirs.clone()})
        })
        .collect();
    serde_json::Value::Array(items).to_string()
}

pub fn generate_traversal_dirs(resolved: &ResolvedScope) -> Vec<String> {
    let mut dirs: Vec<String> = generate_scope_elements(resolved)
        .into_iter()
        .flat_map(|element| element.dirs)
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

pub fn plan_generate(
    resolved: &ResolvedScope,
    bazel_options: &[String],
    check: bool,
    startup: &[String],
) -> Result<BuildPlan, ForwardError> {
    let required = Vec::new();
    let protected = Vec::new();
    let target = if check {
        GENERATE_CHECK_TARGET
    } else {
        GENERATE_TARGET
    };
    let mut argv = build_workflow_argv(
        "run",
        bazel_options,
        &required,
        &protected,
        &[target.to_owned()],
        startup,
    )?;
    let dirs = generate_traversal_dirs(resolved);
    let root_only = dirs.len() == 1 && dirs.first().is_some_and(String::is_empty);
    if !dirs.is_empty() && !root_only {
        argv.push("--".to_owned());
        argv.extend(dirs);
    }
    let (scope, _) = workflow_scope_labels(resolved);
    let summary = format!("Running generate for {}", describe_scope(&scope));
    Ok(BuildPlan { argv, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;
    use dx_process::Scope;

    fn resolved(targets: &[&str]) -> ResolvedScope {
        ResolvedScope {
            scope: if targets.is_empty() {
                Scope::Repository
            } else {
                Scope::Labels(strings(targets))
            },
            targets: strings(targets),
        }
    }

    #[test]
    fn generate_plan_runs_canonical_runner_repo_wide() {
        assert_eq!(GENERATE_TARGET, "//dx:generate");
        assert_eq!(GENERATE_CHECK_TARGET, "//dx:generate_check");
        let plan = plan_generate(&resolved(&[]), &strings(&["--jobs=4"]), false, &[]).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--jobs=4",
                "//dx:generate",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let bare = plan_generate(&resolved(&[]), &[], false, &[]).expect("plan");
        assert_eq!(
            bare.argv.last(),
            Some(&GENERATE_TARGET.to_owned()),
            "{bare:?}"
        );
    }

    #[test]
    fn generate_plan_check_selects_non_mutating_runner() {
        let plan = plan_generate(&resolved(&[]), &[], true, &[]).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "//dx:generate_check",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let scoped = plan_generate(&resolved(&["//a:one"]), &[], true, &[]).expect("plan");
        assert_eq!(
            scoped.argv.last(),
            Some(&"a".to_owned()),
            "check keeps scoped traversal: {scoped:?}"
        );
        assert!(
            scoped.argv.contains(&GENERATE_CHECK_TARGET.to_owned()),
            "{scoped:?}"
        );
    }

    #[test]
    fn generate_plan_forwards_the_consumer_policy_selection() {
        let selection = "--@rules_dx//config:workspace=//consumer:policy";
        let plan = plan_generate(&resolved(&[]), &strings(&[selection]), false, &[]).expect("plan");
        assert!(
            plan.argv.contains(&selection.to_owned()),
            "the consumer policy selection reaches bazel: {plan:?}"
        );
    }

    #[test]
    fn generate_plan_rejects_startup_options() {
        let err = plan_generate(&resolved(&[]), &strings(&["--home_rc"]), false, &[])
            .expect_err("startup option must fail");
        assert!(matches!(err, ForwardError::StartupOption { .. }));
    }

    #[test]
    fn generate_plan_forwards_scoped_traversal_dirs() {
        let plan = plan_generate(&resolved(&["//b/...", "//a:one"]), &[], false, &[]).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "//dx:generate",
                "--",
                "a",
                "b",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //b/... //a:one");
    }

    #[test]
    fn generate_plan_root_package_label_stays_repo_wide() {
        let plan = plan_generate(&resolved(&["//:foo"]), &[], false, &[]).expect("plan");
        assert!(
            !plan.argv.contains(&"--".to_owned()),
            "root traversal needs no positional arguments: {plan:?}"
        );
        assert_eq!(plan.summary, "Running generate for //:foo");
    }

    #[test]
    fn generate_scope_json_matches_manifest_contract() {
        let json = generate_scope_json(&resolved(&["//a:one", "//b/..."]));
        assert_eq!(
            json,
            r#"[{"dirs":["a"],"element":"//a:one"},{"dirs":["b"],"element":"//b/..."}]"#
        );
        assert_eq!(
            generate_scope_json(&resolved(&[])),
            r#"[{"dirs":[""],"element":"//..."}]"#
        );
    }

    #[test]
    fn generate_traversal_dirs_sort_dedup_and_fall_back() {
        assert_eq!(
            generate_traversal_dirs(&resolved(&["//b/...", "//b/...", "//a:one"])),
            strings(&["a", "b"])
        );
        assert_eq!(generate_traversal_dirs(&resolved(&[])), vec![String::new()]);
        assert_eq!(
            generate_traversal_dirs(&resolved(&["@ext//pkg/..."])),
            vec![String::new()]
        );
        assert_eq!(
            generate_traversal_dirs(&resolved(&["//a"])),
            strings(&["a"])
        );
        let resolved_owners = ResolvedScope {
            scope: Scope::ResolvedOwners(strings(&["//a:one"])),
            targets: strings(&["//a:one"]),
        };
        assert_eq!(
            generate_scope_elements(&resolved_owners),
            &[GenerateScopeElement {
                element: "//a:one".to_owned(),
                dirs: vec!["a".to_owned()],
            }]
        );
    }
}
