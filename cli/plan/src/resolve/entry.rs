use std::path::Path;

use dx_process::Scope;

use super::{
    classify_scopes, owners_in_graph, tests_for_owners, PackageCache, QueryRunner, ResolveError,
    ResolvedScope, RuleGraph, SelectionContext,
};

pub fn resolve(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<ResolvedScope, ResolveError> {
    if scopes.is_empty() {
        return Ok(ResolvedScope {
            scope: Scope::Repository,
            targets: vec!["//...".to_owned()],
        });
    }
    let mut cache = PackageCache::default();
    let classified = classify_scopes(scopes, workspace, &mut cache)?;
    if classified.files.is_empty() && classified.patterns.is_empty() {
        return Ok(ResolvedScope {
            scope: Scope::Labels(classified.labels.clone()),
            targets: classified.labels,
        });
    }
    let mut targets = classified.labels;
    if !classified.files.is_empty() {
        let graph = RuleGraph::load(workspace, runner, startup_options)?;
        targets.extend(owners_in_graph(&classified.files, &graph)?);
    }
    targets.extend(classified.patterns);
    targets.sort();
    targets.dedup();
    Ok(ResolvedScope {
        scope: Scope::ResolvedOwners(targets.clone()),
        targets,
    })
}

pub fn resolve_for_test_with_selection(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    selection: &SelectionContext,
) -> Result<ResolvedScope, ResolveError> {
    if scopes.is_empty() {
        return Ok(ResolvedScope {
            scope: Scope::Repository,
            targets: vec!["//...".to_owned()],
        });
    }
    let mut cache = PackageCache::default();
    let classified = classify_scopes(scopes, workspace, &mut cache)?;
    if classified.files.is_empty() {
        if classified.patterns.is_empty() {
            return Ok(ResolvedScope {
                scope: Scope::Labels(classified.labels.clone()),
                targets: classified.labels,
            });
        }
        let mut targets = classified.labels;
        targets.extend(classified.patterns);
        targets.sort();
        targets.dedup();
        return Ok(ResolvedScope {
            scope: Scope::ResolvedOwners(targets.clone()),
            targets,
        });
    }
    let graph = RuleGraph::load(workspace, runner, startup_options)?;
    let file_owners = owners_in_graph(&classified.files, &graph)?;
    let tests = tests_for_owners(&file_owners, &graph, workspace, runner, selection)?;
    let mut targets = classified.labels;
    targets.extend(classified.patterns);
    targets.extend(tests);
    targets.sort();
    targets.dedup();
    Ok(ResolvedScope {
        scope: Scope::ResolvedOwners(targets.clone()),
        targets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{NeverQuery, QueryResult, QueryRunner};
    use dx_testing::cli::{graph_rule, graph_stdout, strings};
    use std::cell::RefCell;
    use std::path::PathBuf;

    struct FakeQuery {
        calls: RefCell<Vec<(Vec<String>, PathBuf)>>,
        outputs: RefCell<Vec<QueryResult>>,
    }

    impl FakeQuery {
        fn new(outputs: Vec<QueryResult>) -> Self {
            FakeQuery {
                calls: RefCell::new(Vec::new()),
                outputs: RefCell::new(outputs),
            }
        }

        fn ok(lines: &str) -> QueryResult {
            QueryResult {
                code: Some(0),
                stdout: lines.as_bytes().to_vec(),
                stderr: Vec::new(),
            }
        }

        fn calls(&self) -> Vec<(Vec<String>, PathBuf)> {
            self.calls.borrow().clone()
        }
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
            self.calls
                .borrow_mut()
                .push((argv.to_vec(), cwd.to_path_buf()));
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }
    fn write(workspace: &Path, rel: &str, text: &str) {
        let full = workspace.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write file");
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
