use std::path::Path;

use dx_process::Scope;

use super::{
    classify_scopes, map_owners_to_tests, resolve_file_owners, PackageCache, QueryRunner,
    ResolveError, ResolvedScope,
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
        targets.extend(resolve_file_owners(
            &classified.files,
            workspace,
            runner,
            startup_options,
        )?);
    }
    targets.extend(classified.patterns);
    targets.sort();
    targets.dedup();
    Ok(ResolvedScope {
        scope: Scope::ResolvedOwners(targets.clone()),
        targets,
    })
}

pub fn resolve_for_test(
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
    let file_owners = resolve_file_owners(&classified.files, workspace, runner, startup_options)?;
    let tests = map_owners_to_tests(&file_owners, workspace, runner, startup_options)?;
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
    use crate::test_support::strings;
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
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::ok("//pkg:unit\n"),
        ]);
        let got = resolve_for_test(
            &strings(&["pkg/a.py", "//other/..."]),
            &workspace,
            &query,
            &[],
        )
        .expect("resolve");
        assert_eq!(
            got.targets,
            strings(&["//other/...", "//pkg:unit"]),
            "owners are replaced by mapped tests"
        );
        assert_eq!(
            query.calls().len(),
            2,
            "one ownership query plus one mapping"
        );
    }

    #[test]
    fn test_scope_with_orphan_file_reports_no_owner() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-orphan-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::ok("//pkg:a.py\n"),
        ]);
        let err = resolve_for_test(
            &strings(&["pkg/a.py", "pkg/orphan.py"]),
            &workspace,
            &query,
            &[],
        )
        .expect_err("partial ownership must fail");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/orphan.py"]),
                labels: strings(&["//pkg:orphan.py"]),
            }
        );
        assert_eq!(query.calls().len(), 2);
    }

    #[test]
    fn test_scope_without_files_matches_plain_resolve() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-labels-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        let got = resolve_for_test(&strings(&["//a:one", "//b/..."]), &workspace, &query, &[])
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
        let got = resolve_for_test(&strings(&["app"]), &workspace, &query, &[]).expect("dir");
        assert_eq!(got.targets, strings(&["//app/..."]));
    }
}
