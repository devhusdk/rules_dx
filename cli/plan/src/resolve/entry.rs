use std::path::Path;

use dx_process::Scope;

use super::{
    PackageCache, QueryRunner, ResolveError, ResolvedScope, RuleGraph, SelectionContext,
    classify_generate_scopes, classify_scopes, exclude, owners_in_graph, tests_for_owners,
};

/// Expands `scopes` into Bazel targets, leaving out the `dx.toml` excludes.
pub fn resolve(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<ResolvedScope, ResolveError> {
    let excludes = exclude::load(workspace)?;
    let resolved = expand(scopes, workspace, runner, startup_options)?;
    exclude::apply(resolved, &excludes, scopes)
}

/// Like [`resolve`], but files and directories map to their package directory
/// without a Bazel query, so new files and packages without a BUILD file work.
pub fn resolve_for_generate(
    scopes: &[String],
    workspace: &Path,
) -> Result<ResolvedScope, ResolveError> {
    let excludes = exclude::load(workspace)?;
    let resolved = if scopes.is_empty() {
        whole_repo()
    } else if scopes.iter().all(|scope| scope.starts_with("//")) {
        labels(classify_generate_scopes(scopes, workspace)?)
    } else {
        owners(classify_generate_scopes(scopes, workspace)?)
    };
    exclude::apply(resolved, &excludes, scopes)
}

/// Like [`resolve`], but files map to the tests that depend on their owners.
pub fn resolve_for_test_with_selection(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    selection: &SelectionContext,
) -> Result<ResolvedScope, ResolveError> {
    let excludes = exclude::load(workspace)?;
    let resolved = expand_for_test(scopes, workspace, runner, startup_options, selection)?;
    exclude::apply(resolved, &excludes, scopes)
}

fn whole_repo() -> ResolvedScope {
    ResolvedScope {
        scope: Scope::Repository,
        targets: vec!["//...".to_owned()],
        excluded: Vec::new(),
        kept: Vec::new(),
    }
}

fn owners(targets: Vec<String>) -> ResolvedScope {
    ResolvedScope {
        scope: Scope::ResolvedOwners(targets.clone()),
        targets,
        excluded: Vec::new(),
        kept: Vec::new(),
    }
}

fn labels(labels: Vec<String>) -> ResolvedScope {
    ResolvedScope {
        scope: Scope::Labels(labels.clone()),
        targets: labels,
        excluded: Vec::new(),
        kept: Vec::new(),
    }
}

fn expand(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<ResolvedScope, ResolveError> {
    if scopes.is_empty() {
        return Ok(whole_repo());
    }
    let mut cache = PackageCache::default();
    let classified = classify_scopes(scopes, workspace, &mut cache)?;
    if classified.files.is_empty() && classified.patterns.is_empty() {
        return Ok(labels(classified.labels));
    }
    let mut targets = classified.labels;
    if !classified.files.is_empty() {
        let graph = RuleGraph::load(workspace, runner, startup_options)?;
        targets.extend(owners_in_graph(&classified.files, &graph)?);
    }
    targets.extend(classified.patterns);
    targets.sort();
    targets.dedup();
    Ok(owners(targets))
}

fn expand_for_test(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    selection: &SelectionContext,
) -> Result<ResolvedScope, ResolveError> {
    if scopes.is_empty() {
        return Ok(whole_repo());
    }
    let mut cache = PackageCache::default();
    let classified = classify_scopes(scopes, workspace, &mut cache)?;
    if classified.files.is_empty() {
        if classified.patterns.is_empty() {
            return Ok(labels(classified.labels));
        }
        let mut targets = classified.labels;
        targets.extend(classified.patterns);
        targets.sort();
        targets.dedup();
        return Ok(owners(targets));
    }
    let graph = RuleGraph::load(workspace, runner, startup_options)?;
    let file_owners = owners_in_graph(&classified.files, &graph)?;
    let tests = tests_for_owners(&file_owners, &graph, workspace, runner, selection)?;
    let mut targets = classified.labels;
    targets.extend(classified.patterns);
    targets.extend(tests);
    targets.sort();
    targets.dedup();
    Ok(owners(targets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{FakeQuery, NeverQuery, QueryResult};
    use dx_test_scratch::write;
    use dx_testing::cli::{graph_rule, graph_stdout, strings};
    use std::cell::RefCell;

    #[test]
    fn generate_scope_maps_new_files_and_dirs_without_a_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-generate-scope-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "svc/new.py", "x = 1\n");
        write(&workspace, "top.py", "x = 1\n");
        write(&workspace, "fresh/deep/a.txt", "x");
        let got = resolve_for_generate(&strings(&["svc/new.py"]), &workspace).expect("file");
        assert_eq!(got.targets, strings(&["//svc/..."]));
        let got = resolve_for_generate(&strings(&["top.py"]), &workspace).expect("root file");
        assert_eq!(got.targets, strings(&["//..."]));
        let got = resolve_for_generate(&strings(&["fresh", "//other/..."]), &workspace)
            .expect("dir and label");
        assert_eq!(got.targets, strings(&["//fresh/...", "//other/..."]));
        let err = resolve_for_generate(&strings(&["missing.py"]), &workspace).unwrap_err();
        assert!(matches!(err, ResolveError::PathNotFound { .. }), "{err:?}");
    }

    #[test]
    fn test_scope_maps_files_to_tests_and_keeps_patterns() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&graph_stdout(&[
                graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
                graph_rule("//pkg:unit", &["//pkg:lib"], &[]),
            ])),
            FakeQuery::ok("//pkg:unit\n"),
        ]);
        write(&workspace, "other/BUILD.bazel", "");
        let got = resolve_for_test_with_selection(
            &strings(&["pkg/a.py", "//other/..."]),
            &workspace,
            &query,
            &[],
            &SelectionContext::unconfigured(&[]),
        )
        .expect("resolve");
        assert_eq!(
            got.targets,
            strings(&["//other/...", "//pkg:unit"]),
            "owners are replaced by mapped tests"
        );
        assert_eq!(query.calls().len(), 2, "one graph query plus one mapping");
    }

    #[test]
    fn test_scope_with_orphan_file_reports_no_owner() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-orphan-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[graph_rule(
            "//pkg:lib",
            &["//pkg:a.py"],
            &[],
        )]))]);
        let err = resolve_for_test_with_selection(
            &strings(&["pkg/a.py", "pkg/orphan.py"]),
            &workspace,
            &query,
            &[],
            &SelectionContext::unconfigured(&[]),
        )
        .expect_err("partial ownership must fail");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/orphan.py"]),
                labels: strings(&["//pkg:orphan.py"]),
            }
        );
        assert_eq!(query.calls().len(), 1);
    }

    struct Routed {
        expressions: RefCell<Vec<String>>,
    }

    impl QueryRunner for Routed {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> std::io::Result<QueryResult> {
            let expression = match argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--query_file="))
            {
                Some(path) => std::fs::read_to_string(path)?,
                None => argv.last().cloned().unwrap_or_default(),
            };
            self.expressions.borrow_mut().push(expression.clone());
            let (code, stdout) =
                if expression.contains("rdeps(//...") || expression.contains("@@android") {
                    (7, String::new())
                } else if expression == "kind('rule', //...)" {
                    (
                        0,
                        graph_stdout(&[
                            graph_rule("//pkg:lib", &["//pkg:a.py", "//pkg:b.py"], &[]),
                            graph_rule("//pkg:unit", &["//pkg:lib"], &[]),
                            graph_rule("//pkg:e2e", &["//pkg:lib", "@@android+sdk//:core"], &[]),
                        ]),
                    )
                } else if expression.starts_with("tests(") {
                    (0, "//pkg:e2e\n//pkg:unit\n".to_owned())
                } else {
                    (
                        0,
                        "//pkg:lib (abc)\n//pkg:e2e (abc)\n//pkg:unit (abc)\n".to_owned(),
                    )
                };
            Ok(QueryResult {
                code: Some(code),
                stdout: stdout.into_bytes(),
                stderr: b"ERROR: no such package '@@android+sdk//'\n".to_vec(),
            })
        }
    }

    fn selections() -> [SelectionContext; 2] {
        [
            SelectionContext::unconfigured(&[]),
            SelectionContext::for_tests_configured(&[], &[]),
        ]
    }

    #[test]
    fn file_scopes_never_query_rdeps_over_the_workspace() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-no-rdeps-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        for selection in selections() {
            let query = Routed {
                expressions: RefCell::new(Vec::new()),
            };
            resolve_for_test_with_selection(
                &strings(&["pkg/a.py"]),
                &workspace,
                &query,
                &[],
                &selection,
            )
            .expect("resolve");
            let expressions = query.expressions.borrow();
            assert_eq!(expressions[0], "kind('rule', //...)");
            for expression in &expressions[1..] {
                assert!(!expression.contains("rdeps(//"), "{expression}");
                assert!(!expression.contains("//..."), "{expression}");
            }
            let expected = if selection.configured() { 3 } else { 2 };
            assert_eq!(expressions.len(), expected, "{expressions:?}");
        }
    }

    #[test]
    fn an_unloadable_external_repo_does_not_fail_a_file_scope() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-external-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        for selection in selections() {
            let query = Routed {
                expressions: RefCell::new(Vec::new()),
            };
            let got = resolve_for_test_with_selection(
                &strings(&["pkg/a.py"]),
                &workspace,
                &query,
                &[],
                &selection,
            )
            .expect("resolve");
            assert_eq!(got.targets, strings(&["//pkg:e2e", "//pkg:unit"]));
            assert_eq!(got.scope, Scope::ResolvedOwners(got.targets.clone()));
        }
    }

    #[test]
    fn file_scopes_resolve_owners_without_loading_external_repos() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-file-scope-external-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = Routed {
            expressions: RefCell::new(Vec::new()),
        };
        let got =
            resolve(&strings(&["pkg/a.py", "pkg/b.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:lib"]));
        assert_eq!(got.scope, Scope::ResolvedOwners(strings(&["//pkg:lib"])));
        assert_eq!(
            *query.expressions.borrow(),
            strings(&["kind('rule', //...)"])
        );
    }

    #[test]
    fn test_scope_without_files_matches_plain_resolve() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-labels-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        write(&workspace, "b/BUILD.bazel", "");
        let got = resolve_for_test_with_selection(
            &strings(&["//a:one", "//b/..."]),
            &workspace,
            &query,
            &[],
            &SelectionContext::unconfigured(&[]),
        )
        .expect("resolve");
        assert_eq!(got.targets, strings(&["//a:one", "//b/..."]));
        assert_eq!(got.scope, Scope::Labels(strings(&["//a:one", "//b/..."])));
    }

    #[test]
    fn test_scope_dir_only_resolves_without_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-dir-");
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("app")).expect("dir");
        let query = NeverQuery;
        let got = resolve_for_test_with_selection(
            &strings(&["app"]),
            &workspace,
            &query,
            &[],
            &SelectionContext::unconfigured(&[]),
        )
        .expect("dir");
        assert_eq!(got.targets, strings(&["//app/..."]));
    }
}
