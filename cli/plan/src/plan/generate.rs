use dx_error::DxError;
use dx_path::label::{Label, Repo};
use dx_process::describe_scope;

use super::{BuildPlan, bazel_argv, rules_dx_label, workflow_scope_labels};
use crate::resolve::ResolvedScope;

/// The `rules_dx` target `dx generate` runs, after the repository.
pub const GENERATE_TARGET: &str = "dx:generate";
/// The `rules_dx` target `dx generate --check` runs, after the repository.
pub const GENERATE_CHECK_TARGET: &str = "dx:generate_check";

#[must_use]
pub fn generate_target(check: bool) -> String {
    rules_dx_label(if check {
        GENERATE_CHECK_TARGET
    } else {
        GENERATE_TARGET
    })
}

pub const GENERATE_ENV_INTENDED: &str = "DX_GENERATE_INTENDED";
pub const GENERATE_ENV_SCOPE: &str = "DX_GENERATE_SCOPE";
pub const GENERATE_ENV_MODE: &str = "DX_GENERATE_MODE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateScopeElement {
    pub element: String,
    pub dirs: Vec<String>,
}

/// One directory Gazelle visits, and whether it visits the packages below it too.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Traversal {
    /// Workspace-relative; `""` is the root package.
    dir: String,
    recursive: bool,
}

impl Traversal {
    fn repo_wide() -> Self {
        Traversal {
            dir: String::new(),
            recursive: true,
        }
    }

    fn is_repo_wide(&self) -> bool {
        self.recursive && self.dir.is_empty()
    }

    /// Whether `self` visits every package `other` visits.
    fn covers(&self, other: &Traversal) -> bool {
        if self.dir == other.dir {
            return self.recursive || !other.recursive;
        }
        self.recursive && (self.dir.is_empty() || other.dir.starts_with(&format!("{}/", self.dir)))
    }

    fn pattern(&self) -> String {
        match (self.recursive, self.dir.is_empty()) {
            (true, true) => "//...".to_owned(),
            (true, false) => format!("//{}/...", self.dir),
            (false, _) => format!("//{}:all", self.dir),
        }
    }
}

/// Maps one scope label to what Gazelle traverses.
///
/// `//pkg/...` is the `pkg` tree; any other label in the main repository is its
/// package alone, so `//pkg:x` and `//:x` never reach the packages below.
/// `@//` and `@@//` name the main repository and are read as `//`. Any other
/// `@repo//` label falls back to repo-wide: Gazelle cannot walk an external
/// repository, and `dx generate` refuses such labels at parse time, so only
/// the `--affected` umbrella path reaches this fallback, where widening is
/// the safe direction.
fn generate_traversal(target: &str) -> Traversal {
    let Some(label) = Label::parse(target) else {
        return Traversal::repo_wide();
    };
    match label.repo {
        Repo::Local | Repo::Apparent("") | Repo::Canonical("") if !label.relative => {}
        Repo::Local | Repo::Apparent(_) | Repo::Canonical(_) => return Traversal::repo_wide(),
    }
    // `//...` and `//pkg/...` keep the wildcard in the package; the name (`all`,
    // `*`, `all-targets` or a target) never narrows the directory further.
    if label.package == "..." {
        return Traversal::repo_wide();
    }
    match label.package.strip_suffix("/...") {
        Some(dir) => Traversal {
            dir: dir.to_owned(),
            recursive: true,
        },
        None => Traversal {
            dir: label.package.to_owned(),
            recursive: false,
        },
    }
}

fn generate_traversal_dir(target: &str) -> String {
    generate_traversal(target).dir
}

/// Sorted traversals with every one another already visits left out.
fn collapse(mut traversals: Vec<Traversal>) -> Vec<Traversal> {
    if traversals.iter().any(Traversal::is_repo_wide) {
        return vec![Traversal::repo_wide()];
    }
    traversals.sort();
    traversals.dedup();
    let kept: Vec<Traversal> = traversals
        .iter()
        .filter(|walk| {
            !traversals
                .iter()
                .any(|other| other != *walk && other.covers(walk))
        })
        .cloned()
        .collect();
    kept
}

/// Collapses target labels into sorted, non-overlapping Gazelle package
/// patterns: `//pkg/...` for a tree, `//pkg:all` for one package.
pub fn generate_package_scope(targets: &[String]) -> Vec<String> {
    collapse(
        targets
            .iter()
            .map(|target| generate_traversal(target))
            .collect(),
    )
    .iter()
    .map(Traversal::pattern)
    .collect()
}

/// The part of the generate `patterns` that lies inside `scope`, as package patterns:
/// `//...` within a `//svc:bin` scope is `//svc:all`, never the whole repository.
pub fn generate_package_scope_within(patterns: &[String], scope: &[String]) -> Vec<String> {
    let scope: Vec<Traversal> = scope
        .iter()
        .map(|entry| generate_traversal(entry))
        .collect();
    let mut kept = Vec::new();
    for pattern in patterns {
        let walk = generate_traversal(pattern);
        for entry in &scope {
            if entry.covers(&walk) {
                kept.push(walk.clone());
            } else if walk.covers(entry) {
                kept.push(entry.clone());
            }
        }
    }
    collapse(kept).iter().map(Traversal::pattern).collect()
}

#[must_use]
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

#[must_use]
pub fn generate_scope_json(resolved: &ResolvedScope) -> String {
    let items: Vec<serde_json::Value> = generate_scope_elements(resolved)
        .iter()
        .map(|element| {
            serde_json::json!({"element": element.element.clone(), "dirs": element.dirs.clone()})
        })
        .collect();
    serde_json::Value::Array(items).to_string()
}

fn generate_traversals(resolved: &ResolvedScope) -> Vec<Traversal> {
    let (_, labels) = workflow_scope_labels(resolved);
    collapse(
        labels
            .iter()
            .map(|target| generate_traversal(target))
            .collect(),
    )
}

/// Whether the generate walk of `resolved` visits the package in `dir`; `.` is the root.
#[must_use]
pub fn generate_scope_visits(resolved: &ResolvedScope, dir: &str) -> bool {
    let package = Traversal {
        dir: if dir == "." {
            String::new()
        } else {
            dir.to_owned()
        },
        recursive: false,
    };
    generate_traversals(resolved)
        .iter()
        .any(|walk| walk.covers(&package))
}

#[must_use]
pub fn generate_traversal_dirs(resolved: &ResolvedScope) -> Vec<String> {
    generate_traversals(resolved)
        .into_iter()
        .map(|walk| walk.dir)
        .collect()
}

/// Workspace-relative directories under `[scope] exclude`, as Gazelle `-exclude` values.
///
/// Only recursive patterns count: a single-package or external pattern must
/// not hide a whole directory tree from the generator.
pub fn generate_exclude_dirs(resolved: &ResolvedScope) -> Vec<String> {
    let mut dirs: Vec<String> = resolved
        .excluded
        .iter()
        .filter_map(|pattern| dx_config::conventions::recursive_dir(pattern))
        .map(str::to_owned)
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn plan_generate(
    resolved: &ResolvedScope,
    bazel_options: &[String],
    check: bool,
    startup_options: &[String],
    patch: Option<&str>,
) -> Result<BuildPlan, DxError> {
    let mut argv = bazel_argv(
        "run",
        bazel_options,
        &[],
        &[],
        &[generate_target(check)],
        startup_options,
    )?;
    let walks = generate_traversals(resolved);
    let root_only = walks.iter().any(Traversal::is_repo_wide);
    let patch = patch.filter(|_| check);
    let excludes = if root_only {
        generate_exclude_dirs(resolved)
    } else {
        Vec::new()
    };
    if patch.is_some() || !excludes.is_empty() || (!walks.is_empty() && !root_only) {
        argv.push("--".to_owned());
        if let Some(patch) = patch {
            argv.push(format!("-patch={patch}"));
        }
        argv.extend(excludes.iter().map(|dir| format!("-exclude={dir}")));
        if !root_only {
            // Gazelle has one recursion switch for every directory it is given, so a
            // scope that names any tree visits the packages below a lone package too.
            if !walks.iter().any(|walk| walk.recursive) {
                argv.push("-r=false".to_owned());
            }
            argv.extend(walks.into_iter().map(|walk| {
                if walk.dir.is_empty() {
                    ".".to_owned()
                } else {
                    walk.dir
                }
            }));
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
            excluded: Vec::new(),
            kept: Vec::new(),
        }
    }

    #[test]
    fn generate_plan_runs_canonical_runner_repo_wide() {
        assert_eq!(generate_target(false), "@rules_dx//dx:generate");
        assert_eq!(generate_target(true), "@rules_dx//dx:generate_check");
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
                "@rules_dx//dx:generate",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let bare = plan_generate(&resolved(&[]), &[], false, &[], None).expect("plan");
        assert_eq!(bare.argv.last(), Some(&generate_target(false)), "{bare:?}");
    }

    #[test]
    fn generate_plan_over_the_whole_repo_passes_scope_excludes() {
        let mut scope = resolved(&[]);
        scope.excluded = strings(&["//third_party/...", "//a/b/...", "//x:y", "@ext//..."]);
        let plan = plan_generate(&scope, &[], false, &[], None).expect("plan");
        assert_eq!(
            plan.argv[plan.argv.len() - 3..],
            strings(&["--", "-exclude=a/b", "-exclude=third_party"])
        );
        let scoped = ResolvedScope {
            excluded: strings(&["//third_party/..."]),
            ..resolved(&["//a:one"])
        };
        let plan = plan_generate(&scoped, &[], false, &[], None).expect("plan");
        assert_eq!(
            plan.argv[plan.argv.len() - 3..],
            strings(&["--", "-r=false", "a"])
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
                "@rules_dx//dx:generate_check",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //...");
        let scoped = plan_generate(&resolved(&["//a:one"]), &[], true, &[], None).expect("plan");
        assert_eq!(
            scoped.argv.last(),
            Some(&"a".to_owned()),
            "check keeps scoped traversal: {scoped:?}"
        );
        assert!(scoped.argv.contains(&generate_target(true)), "{scoped:?}");
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
            scoped.argv[scoped.argv.len() - 4..],
            [
                "--".to_owned(),
                patch,
                "-r=false".to_owned(),
                "a".to_owned()
            ]
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
        assert!(err.message().starts_with("invalid_usage: --"));
    }

    #[test]
    fn generate_plan_forwards_scoped_traversal_dirs() {
        let plan =
            plan_generate(&resolved(&["//b/...", "//a:one"]), &[], false, &[], None).expect("plan");
        // A tree in the scope keeps Gazelle recursive for every directory.
        assert_eq!(
            plan.argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "run",
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
                "@rules_dx//dx:generate",
                "--",
                "a",
                "b",
            ])
        );
        assert_eq!(plan.summary, "Running generate for //b/... //a:one");
    }

    #[test]
    fn generate_plan_root_package_label_visits_only_the_root_package() {
        let plan = plan_generate(&resolved(&["//:foo"]), &[], false, &[], None).expect("plan");
        assert_eq!(
            plan.argv[plan.argv.len() - 3..],
            strings(&["--", "-r=false", "."])
        );
        assert_eq!(plan.summary, "Running generate for //:foo");
        let plan = plan_generate(&resolved(&["//..."]), &[], false, &[], None).expect("plan");
        assert!(
            !plan.argv.contains(&"--".to_owned()),
            "root traversal needs no positional arguments: {plan:?}"
        );
    }

    #[test]
    fn generate_plan_for_packages_leaves_their_subpackages_alone() {
        let plan = plan_generate(
            &resolved(&["//cli/cli/src/exec:all", "//:all", "//cli/cli/src/exec:x"]),
            &[],
            false,
            &[],
            None,
        )
        .expect("plan");
        assert_eq!(
            plan.argv[plan.argv.len() - 4..],
            strings(&["--", "-r=false", ".", "cli/cli/src/exec"])
        );
    }

    #[test]
    fn generate_scope_visits_only_the_packages_the_walk_reaches() {
        let whole = resolved(&[]);
        assert!(generate_scope_visits(&whole, "."));
        assert!(generate_scope_visits(&whole, "php/app"));
        let scoped = resolved(&["//a:one", "//b/..."]);
        assert!(generate_scope_visits(&scoped, "a"));
        assert!(!generate_scope_visits(&scoped, "a/sub"));
        assert!(generate_scope_visits(&scoped, "b"));
        assert!(generate_scope_visits(&scoped, "b/sub"));
        assert!(!generate_scope_visits(&scoped, "bc"));
        assert!(!generate_scope_visits(&scoped, "."));
        assert!(generate_scope_visits(&resolved(&["//:all"]), "."));
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
            ("@@ext+//p:n", ""),
            ("@ext", ""),
            (":n", ""),
            ("pkg:n", ""),
        ] {
            assert_eq!(generate_traversal_dir(label), dir, "{label}");
        }
    }

    #[test]
    fn scoped_all_targets_never_widen_to_the_root_package() {
        let resolved = resolved(&["//cli/cli:all", "//docs:all"]);
        assert_eq!(
            generate_traversal_dirs(&resolved),
            strings(&["cli/cli", "docs"])
        );
        let plan = plan_generate(&resolved, &[], false, &[], None).expect("plan");
        let tail: Vec<&str> = plan.argv.iter().rev().take(4).map(String::as_str).collect();
        assert_eq!(tail, ["docs", "cli/cli", "-r=false", "--"]);
    }

    #[test]
    fn generate_package_scope_collapses_labels_to_packages() {
        assert_eq!(
            generate_package_scope(&strings(&["//a:x", "//a:y", "//b/c:z"])),
            strings(&["//a:all", "//b/c:all"])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//a/...", "//a/b:x", "//a:y"])),
            strings(&["//a/..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//a:x", "//a/b:y", "//a/...:all"])),
            strings(&["//a/..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//ab:x", "//a:y", "//a/b:z"])),
            strings(&["//a:all", "//a/b:all", "//ab:all"])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//a:x", "//..."])),
            strings(&["//..."])
        );
        assert_eq!(
            generate_package_scope(&strings(&["//:root", "//b:y"])),
            strings(&["//:all", "//b:all"])
        );
        assert_eq!(
            generate_package_scope(&strings(&["@ext//p:t", "//b:y"])),
            strings(&["//..."])
        );
        assert_eq!(generate_package_scope(&[]), [] as [String; 0]);
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
            excluded: Vec::new(),
            kept: Vec::new(),
        };
        assert_eq!(
            generate_scope_elements(&resolved_owners),
            &[GenerateScopeElement {
                element: "//a:one".to_owned(),
                dirs: vec!["a".to_owned()],
            }]
        );
    }

    #[test]
    fn generate_package_scope_within_keeps_the_overlap() {
        let within = |patterns: &[&str], scope: &[&str]| {
            generate_package_scope_within(&strings(patterns), &strings(scope))
        };
        assert_eq!(
            within(&["//..."], &["//svc:bin", "//web/..."]),
            strings(&["//svc:all", "//web/..."])
        );
        assert_eq!(
            within(&["//svc/...", "//web:all"], &["//svc/api:lib"]),
            strings(&["//svc/api:all"])
        );
        assert_eq!(
            within(&["//svc:all", "//svc/api:all"], &["//svc/..."]),
            strings(&["//svc:all", "//svc/api:all"])
        );
        assert_eq!(
            within(&["//svc:all"], &["//svc/api/...", "//web:all"]),
            [] as [String; 0]
        );
    }

    #[test]
    fn excluded_dirs_are_not_traversed() {
        let resolved = crate::resolve::exclude::apply(
            resolved(&["//app/...", "//third_party/x/..."]),
            &strings(&["//third_party/..."]),
            &strings(&["app", "third_party/x"]),
        )
        .expect("apply");
        assert_eq!(generate_traversal_dirs(&resolved), strings(&["app"]));
    }

    #[test]
    fn every_recursive_exclude_spelling_reaches_gazelle() {
        for pattern in [
            "//vendor/...",
            "//vendor/...:all",
            "//vendor/...:*",
            "//vendor/...:all-targets",
        ] {
            let mut resolved = resolved(&["//app/..."]);
            resolved.excluded = strings(&[pattern, "//pkg:all", "//...", "@ext//vendor/..."]);
            assert_eq!(
                generate_exclude_dirs(&resolved),
                strings(&["vendor"]),
                "{pattern}"
            );
        }
    }
}
