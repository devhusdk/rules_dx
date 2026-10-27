use dx_process::{describe_scope, ForwardError};

use super::{bazel_argv, workflow_scope_labels, BuildPlan};
use crate::resolve::ResolvedScope;

pub(crate) const GENERATE_TARGET: &str = "//dx:generate";
pub(crate) const GENERATE_CHECK_TARGET: &str = "//dx:generate_check";

pub const GENERATE_ENV_INTENDED: &str = "DX_GENERATE_INTENDED";
pub const GENERATE_ENV_SCOPE: &str = "DX_GENERATE_SCOPE";
pub const GENERATE_ENV_MODE: &str = "DX_GENERATE_MODE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateScopeElement {
    pub(crate) element: String,
    pub(crate) dirs: Vec<String>,
}

/// Maps one scope label to the Gazelle directory it traverses ("" is repo-wide).
///
/// `@//` and `@@//` name the main repository and are read as `//`. Any other
/// `@repo//` label falls back to repo-wide: Gazelle cannot walk an external
/// repository, and `dx generate` refuses such labels at parse time, so only
/// the `--affected` umbrella path reaches this fallback, where widening is
/// the safe direction. `//:x` stays repo-wide because Gazelle directory
/// arguments are recursive.
fn generate_traversal_dir(target: &str) -> String {
    let label = target
        .strip_prefix("@@")
        .or_else(|| target.strip_prefix('@'))
        .filter(|rest| rest.starts_with("//"))
        .unwrap_or(target);
    let label = ["all-targets", "all", "*"]
        .iter()
        .find_map(|name| {
            label
                .strip_suffix(name)
                .and_then(|head| head.strip_suffix(':'))
        })
        .unwrap_or(label);
    if label == "..." {
        return String::new();
    }
    let Some(rest) = label.strip_prefix("//") else {
        return String::new();
    };
    if rest == "..." {
        return String::new();
    }
    if let Some(dir) = rest.strip_suffix("/...") {
        return dir.to_owned();
    }
    match rest.split_once(':') {
        Some((package, _)) => package.to_owned(),
        None => rest.to_owned(),
    }
}

/// Collapses target labels into sorted, non-overlapping Gazelle package patterns.
pub(crate) fn generate_package_scope(targets: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = targets
        .iter()
        .map(|target| generate_traversal_dir(target))
        .collect();
    if dirs.iter().any(String::is_empty) {
        return vec!["//...".to_owned()];
    }
    dirs.sort();
    dirs.dedup();
    let mut kept: Vec<String> = Vec::new();
    for dir in dirs {
        let nested = kept
            .iter()
            .any(|parent| dir.starts_with(&format!("{parent}/")));
        if !nested {
            kept.push(dir);
        }
    }
    kept.into_iter().map(|dir| format!("//{dir}/...")).collect()
}

pub(crate) fn generate_scope_elements(resolved: &ResolvedScope) -> Vec<GenerateScopeElement> {
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

pub(crate) fn generate_traversal_dirs(resolved: &ResolvedScope) -> Vec<String> {
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
    startup_options: &[String],
    patch: Option<&str>,
) -> Result<BuildPlan, ForwardError> {
    let required = Vec::new();
    let protected = Vec::new();
    let target = if check {
        GENERATE_CHECK_TARGET
    } else {
        GENERATE_TARGET
    };
    let mut argv = bazel_argv(
        "run",
        bazel_options,
        &required,
        &protected,
        &[target.to_owned()],
        startup_options,
    )?;
    let dirs = generate_traversal_dirs(resolved);
    let root_only = dirs.len() == 1 && dirs.first().is_some_and(String::is_empty);
    let patch = patch.filter(|_| check);
    if patch.is_some() || (!dirs.is_empty() && !root_only) {
        argv.push("--".to_owned());
        if let Some(patch) = patch {
            argv.push(format!("-patch={patch}"));
        }
        if !root_only {
            argv.extend(dirs);
        }
    }
    let (scope, _) = workflow_scope_labels(resolved);
    let summary = format!("Running generate for {}", describe_scope(&scope));
    Ok(BuildPlan { argv, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_process::Scope;
    use dx_testing::cli::strings;

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
        let plan =
            plan_generate(&resolved(&[]), &strings(&["--jobs=4"]), false, &[], None).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--jobs=4",
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
                "//dx:generate",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let bare = plan_generate(&resolved(&[]), &[], false, &[], None).expect("plan");
        assert_eq!(
            bare.argv.last(),
            Some(&GENERATE_TARGET.to_owned()),
            "{bare:?}"
        );
    }

    #[test]
    fn generate_plan_check_selects_non_mutating_runner() {
        let plan = plan_generate(&resolved(&[]), &[], true, &[], None).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
                "//dx:generate_check",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let scoped = plan_generate(&resolved(&["//a:one"]), &[], true, &[], None).expect("plan");
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
    fn generate_plan_check_puts_the_patch_flag_before_the_dirs() {
        let patch = "-patch=/t/x.patch".to_owned();
        let root = plan_generate(&resolved(&[]), &[], true, &[], Some("/t/x.patch")).expect("plan");
        assert_eq!(
            root.argv[root.argv.len() - 2..],
            ["--".to_owned(), patch.clone()]
        );
        let scoped = plan_generate(&resolved(&["//a:one"]), &[], true, &[], Some("/t/x.patch"))
            .expect("plan");
        assert_eq!(
            scoped.argv[scoped.argv.len() - 3..],
            ["--".to_owned(), patch, "a".to_owned()]
        );
        let apply =
            plan_generate(&resolved(&[]), &[], false, &[], Some("/t/x.patch")).expect("plan");
        assert!(!apply.argv.iter().any(|arg| arg.starts_with("-patch=")));
    }

    #[test]
    fn generate_plan_forwards_the_consumer_policy_selection() {
        let selection = "--@rules_dx//config:workspace=//consumer:policy";
        let plan =
            plan_generate(&resolved(&[]), &strings(&[selection]), false, &[], None).expect("plan");
        assert!(
            plan.argv.contains(&selection.to_owned()),
            "the consumer policy selection reaches bazel: {plan:?}"
        );
    }

    #[test]
    fn generate_plan_rejects_startup_options() {
        let err = plan_generate(&resolved(&[]), &strings(&["--home_rc"]), false, &[], None)
            .expect_err("startup option must fail");
        assert!(matches!(err, ForwardError::StartupOption { .. }));
    }

    #[test]
    fn generate_plan_forwards_scoped_traversal_dirs() {
        let plan =
            plan_generate(&resolved(&["//b/...", "//a:one"]), &[], false, &[], None).expect("plan");
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
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
        let plan = plan_generate(&resolved(&["//:foo"]), &[], false, &[], None).expect("plan");
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
    fn generate_traversal_dir_reads_label_forms() {
        for (label, dir) in [
            ("//...", ""),
            ("//...:all", ""),
            ("...", ""),
            ("//pkg/...:all", "pkg"),
            ("//pkg/...:*", "pkg"),
            ("//pkg:all", "pkg"),
            ("//pkg:all-targets", "pkg"),
            ("//pkg:x", "pkg"),
            ("//pkg", "pkg"),
            ("@//pkg:x", "pkg"),
            ("@@//pkg/...", "pkg"),
            ("//:x", ""),
            ("@ext//p", ""),
        ] {
            assert_eq!(generate_traversal_dir(label), dir, "{label}");
        }
    }

    #[test]
    fn generate_package_scope_collapses_labels_to_packages() {
        assert_eq!(
            generate_package_scope(&strings(&["//a:x", "//a:y", "//b/c:z"])),
            strings(&["//a/...", "//b/c/..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//a/...", "//a/b:x"])),
            strings(&["//a/..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//ab:x", "//a:y"])),
            strings(&["//a/...", "//ab/..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//a:x", "//..."])),
            strings(&["//..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//:root", "//b:y"])),
            strings(&["//..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["@ext//p:t", "//b:y"])),
            strings(&["//..."])
        );
        assert!(generate_package_scope(&[]).is_empty());
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
