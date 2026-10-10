use std::io;
use std::path::{Component, Path};

use super::packages::PackageCache;
use super::{
    owned_sources_expression, ownership_set_expression, run_label_query, QueryRunner, ResolveError,
};

fn normalize_rel(raw: &str) -> Result<String, ResolveError> {
    if raw.is_empty() {
        return Err(ResolveError::EmptyScope);
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(ResolveError::OutsideWorkspace {
            scope: raw.to_owned(),
        });
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ResolveError::OutsideWorkspace {
                    scope: raw.to_owned(),
                });
            }
        }
    }
    Ok(parts.join("/"))
}

fn dir_pattern(rel: &str) -> String {
    if rel.is_empty() {
        "//...".to_owned()
    } else {
        format!("//{rel}/...")
    }
}

pub(crate) fn first_line(bytes: &[u8]) -> String {
    dx_output::first_diagnostic_line(bytes, 300, "no Bazel diagnostic")
}

pub(crate) fn parse_owners(stdout: &[u8], label: &str) -> Result<Vec<String>, ResolveError> {
    let text = std::str::from_utf8(stdout).map_err(|_| ResolveError::QueryFailed {
        label: label.to_owned(),
        detail: "query output is not UTF-8".to_owned(),
    })?;
    let mut owners: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect();
    owners.sort();
    owners.dedup();
    Ok(owners)
}

#[derive(Clone)]
pub(crate) struct FileScope {
    pub(crate) scope: String,
    pub(crate) label: String,
}

pub(crate) struct ClassifiedScopes {
    pub(crate) labels: Vec<String>,
    pub(crate) files: Vec<FileScope>,
    pub(crate) patterns: Vec<String>,
    pub(crate) paths: Vec<String>,
}

pub(crate) fn classify_scopes(
    scopes: &[String],
    workspace: &Path,
    cache: &mut PackageCache,
) -> Result<ClassifiedScopes, ResolveError> {
    let mut classified = ClassifiedScopes {
        labels: Vec::new(),
        files: Vec::new(),
        patterns: Vec::new(),
        paths: Vec::new(),
    };
    for raw in scopes {
        if raw.starts_with("//") {
            classified.labels.push(raw.clone());
            continue;
        }
        if raw.starts_with('@') {
            return Err(ResolveError::ExternalScope { scope: raw.clone() });
        }
        if raw.starts_with(':') {
            return Err(ResolveError::RelativeLabel { scope: raw.clone() });
        }
        let rel = normalize_rel(raw)?;
        if rel.chars().any(char::is_control) {
            return Err(ResolveError::UnsupportedName { scope: raw.clone() });
        }
        let entry = workspace.join(&rel);
        let metadata = std::fs::symlink_metadata(&entry).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound
                || error.kind() == io::ErrorKind::NotADirectory
            {
                ResolveError::PathNotFound { scope: raw.clone() }
            } else {
                ResolveError::QueryFailed {
                    label: cache
                        .file_label(workspace, &rel, raw)
                        .unwrap_or_else(|_| raw.clone()),
                    detail: error.to_string(),
                }
            }
        })?;
        if metadata.is_dir() {
            classified.patterns.push(dir_pattern(&rel));
            classified.paths.push(raw.clone());
        } else if metadata.is_file() {
            let label = cache.file_label(workspace, &rel, raw)?;
            classified.files.push(FileScope {
                scope: raw.clone(),
                label,
            });
            classified.paths.push(raw.clone());
        } else {
            return Err(ResolveError::NotFileOrDir { scope: raw.clone() });
        }
    }
    Ok(classified)
}

fn no_owner_error(files: &[FileScope]) -> ResolveError {
    let mut seen = std::collections::HashSet::new();
    let mut scopes = Vec::new();
    let mut labels = Vec::new();
    for file in files {
        if seen.insert((file.scope.as_str(), file.label.as_str())) {
            scopes.push(file.scope.clone());
            labels.push(file.label.clone());
        }
    }
    ResolveError::NoOwner {
        files: scopes,
        labels,
    }
}

pub(crate) fn resolve_file_owners(
    files: &[FileScope],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let labels: Vec<String> = files.iter().map(|file| file.label.clone()).collect();
    let expression = ownership_set_expression(&labels);
    let owners = run_label_query(&expression, workspace, runner, startup_options)?;
    if owners.is_empty() {
        return Err(no_owner_error(files));
    }
    if files.len() > 1 {
        let sources = run_label_query(
            &owned_sources_expression(&owners),
            workspace,
            runner,
            startup_options,
        )?;
        let owned: std::collections::HashSet<&str> = sources.iter().map(String::as_str).collect();
        let missing: Vec<FileScope> = files
            .iter()
            .filter(|file| !owned.contains(file.label.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(no_owner_error(&missing));
        }
    }
    Ok(owners)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::query::quote_label;
    use crate::resolve::{resolve, resolve_for_test, resolve_run};
    use dx_process::Scope;
    use std::cell::RefCell;
    use std::path::PathBuf;

    use crate::resolve::NeverQuery;
    use crate::resolve::QueryResult;
    use crate::test_support::strings;

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
        fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
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

    fn file_label(workspace: &Path, rel: &str, scope: &str) -> Result<String, ResolveError> {
        PackageCache::default().file_label(workspace, rel, scope)
    }

    #[test]
    fn empty_scope_selects_repository() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-empty-");
        let workspace = scratch.path().to_path_buf();
        let got = resolve(&[], &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.scope, Scope::Repository);
        assert_eq!(got.targets, strings(&["//..."]));
    }

    #[test]
    fn label_only_scopes_pass_through_in_order() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-labels-");
        let workspace = scratch.path().to_path_buf();
        let input = strings(&["//b/...", "//a:one", "@repo//c/..."]);
        let err = resolve(&input, &workspace, &query, &[]).expect_err("external must fail");
        assert_eq!(
            err,
            ResolveError::ExternalScope {
                scope: "@repo//c/...".to_owned(),
            }
        );
        let input = strings(&["//b/...", "//a:one"]);
        let got = resolve(&input, &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.scope, Scope::Labels(strings(&["//b/...", "//a:one"])));
        assert_eq!(got.targets, strings(&["//b/...", "//a:one"]));
    }

    #[test]
    fn relative_labels_fail_with_guidance() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-relative-");
        let workspace = scratch.path().to_path_buf();
        let err = resolve(&strings(&[":corpus"]), &workspace, &query, &[]).expect_err("relative");
        assert_eq!(
            err,
            ResolveError::RelativeLabel {
                scope: ":corpus".to_owned(),
            }
        );
        assert!(err.to_string().contains("//"));
    }

    #[test]
    fn file_resolves_through_single_rule_constrained_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-file-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n//pkg:lib\n//pkg:extra\n")]);
        let got = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(
            got.targets,
            strings(&["//pkg:extra", "//pkg:lib"]),
            "owners are sorted and deduplicated"
        );
        assert_eq!(
            got.scope,
            Scope::ResolvedOwners(strings(&["//pkg:extra", "//pkg:lib"]))
        );
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, workspace, "query runs in the workspace");
        assert_eq!(
            calls[0].0,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "query",
                "--",
                "kind('rule', rdeps(//..., set(\"//pkg:a.py\"), 1))",
            ])
        );
    }

    #[test]
    fn file_argv_quotes_spaces_and_special_characters() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-quoting-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/my file.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n")]);
        resolve(&strings(&["pkg/my file.py"]), &workspace, &query, &[]).expect("resolve");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('rule', rdeps(//..., set(\"//pkg:my file.py\"), 1))"
        );
        assert_eq!(quote_label("//pkg:a\"b\\c"), "\"//pkg:a\\\"b\\\\c\"");
    }

    #[test]
    fn multiple_files_share_two_bounded_queries() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n//pkg:extra\n"),
            FakeQuery::ok("//pkg:a.py\n//pkg:b.py\n"),
        ]);
        let got =
            resolve(&strings(&["pkg/b.py", "pkg/a.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:extra", "//pkg:lib"]));
        let calls = query.calls();
        assert_eq!(
            calls.len(),
            2,
            "one ownership query plus one attribution query"
        );
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('rule', rdeps(//..., set(\"//pkg:a.py\" \"//pkg:b.py\"), 1))"
        );
        assert_eq!(
            calls[1].0.last().expect("expression"),
            "deps(set(\"//pkg:extra\" \"//pkg:lib\"), 1)"
        );
    }

    #[test]
    fn empty_batch_mapping_names_every_file() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-empty-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err = resolve(&strings(&["pkg/a.py", "pkg/b.py"]), &workspace, &query, &[])
            .expect_err("orphans");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/a.py", "pkg/b.py"]),
                labels: strings(&["//pkg:a.py", "//pkg:b.py"]),
            }
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn mixed_owned_and_orphan_files_report_every_orphan() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-partial-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/owned.py", "x = 1\n");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        write(&workspace, "pkg/my orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::ok("//pkg:lib\n//pkg:owned.py\n//other:dep\n"),
        ]);
        let err = resolve(
            &strings(&["pkg/owned.py", "pkg/orphan.py", "pkg/my orphan.py"]),
            &workspace,
            &query,
            &[],
        )
        .expect_err("partial ownership must fail");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/orphan.py", "pkg/my orphan.py"]),
                labels: strings(&["//pkg:orphan.py", "//pkg:my orphan.py"]),
            }
        );
        assert!(err.to_string().contains("pkg/orphan.py"), "{err}");
        assert!(err.to_string().contains("pkg/my orphan.py"), "{err}");
        let calls = query.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1].0.last().expect("expression"),
            "deps(set(\"//pkg:lib\"), 1)"
        );
    }

    #[test]
    fn overlapping_owners_cover_every_file() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-overlap-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n//pkg:extra\n"),
            FakeQuery::ok("//pkg:lib\n//pkg:a.py\n//pkg:b.py\n"),
        ]);
        let got =
            resolve(&strings(&["pkg/a.py", "pkg/b.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:extra", "//pkg:lib"]));
        assert_eq!(query.calls().len(), 2);
    }

    #[test]
    fn duplicate_orphan_scopes_are_listed_once() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-dup-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err = resolve(&strings(&["pkg/a.py", "pkg/a.py"]), &workspace, &query, &[])
            .expect_err("orphans");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/a.py"]),
                labels: strings(&["//pkg:a.py"]),
            }
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn attribution_query_failure_stays_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-attrib-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            QueryResult {
                code: Some(2),
                stdout: Vec::new(),
                stderr: b"ERROR: no such package\nmore context\n".to_vec(),
            },
        ]);
        let err = resolve(&strings(&["pkg/a.py", "pkg/b.py"]), &workspace, &query, &[])
            .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "deps(set(\"//pkg:lib\"), 1)".to_owned(),
                detail: "ERROR: no such package".to_owned(),
            }
        );
    }

    #[test]
    fn mixed_labels_and_orphan_files_fail_without_partial_selection() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-mixed-orphan-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err = resolve(
            &strings(&["//z:z", "pkg/orphan.py"]),
            &workspace,
            &query,
            &[],
        )
        .expect_err("orphan");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/orphan.py"]),
                labels: strings(&["//pkg:orphan.py"]),
            }
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn file_after_a_packaged_file_without_package_fails_before_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-no-package-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "docs/guide.md", "# guide\n");
        let query = NeverQuery;
        let err = resolve(
            &strings(&["pkg/a.py", "docs/guide.md"]),
            &workspace,
            &query,
            &[],
        )
        .expect_err("no package");
        assert_eq!(
            err,
            ResolveError::NotAPackage {
                scope: "docs/guide.md".to_owned(),
            }
        );
    }

    #[test]
    fn root_file_maps_to_root_package_label() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-root-file-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "");
        write(&workspace, "top.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//:lib\n")]);
        let got = resolve(&strings(&["top.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//:lib"]));
        let calls = query.calls();
        assert!(calls[0]
            .0
            .last()
            .expect("expression")
            .contains("\"//:top.py\""));
    }

    #[test]
    fn build_file_text_is_never_consulted() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-build-text-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "pkg/BUILD.bazel",
            "# owner: //decoy:not_real\nmy_files = glob([\"*.py\"])\n",
        );
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:real\n")]);
        let got = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:real"]));
    }

    #[test]
    fn directory_becomes_recursive_pattern_without_query_or_listing() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-dir-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "src/nested/deep.py", "x = 1\n");
        write(&workspace, "src/top.py", "x = 1\n");
        let query = NeverQuery;
        let got = resolve(&strings(&["src"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//src/..."]));
        assert_eq!(got.scope, Scope::ResolvedOwners(strings(&["//src/..."])));
    }

    #[test]
    fn workspace_root_directory_maps_to_repository_pattern() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-root-dir-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        for root in [".", "./"] {
            let got = resolve(&strings(&[root]), &workspace, &query, &[]).expect("resolve");
            assert_eq!(got.targets, strings(&["//..."]), "root {root}");
        }
    }

    #[test]
    fn mixed_labels_and_paths_merge_sorted_and_deduplicated() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-mixed-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n//z:z\n")]);
        let got =
            resolve(&strings(&["//z:z", "pkg/a.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:lib", "//z:z"]));
        assert_eq!(
            got.scope,
            Scope::ResolvedOwners(strings(&["//pkg:lib", "//z:z"]))
        );
    }

    #[test]
    fn missing_and_escaping_paths_fail() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-missing-");
        let workspace = scratch.path().to_path_buf();
        assert_eq!(
            resolve(&strings(&["nope.py"]), &workspace, &query, &[]).expect_err("missing"),
            ResolveError::PathNotFound {
                scope: "nope.py".to_owned(),
            }
        );
        for escaping in ["/abs/path.py", "../escape.py", "pkg/../../escape.py"] {
            assert_eq!(
                resolve(&strings(&[escaping]), &workspace, &query, &[]).expect_err("escape"),
                ResolveError::OutsideWorkspace {
                    scope: escaping.to_owned(),
                }
            );
        }
        assert_eq!(
            resolve(&strings(&[""]), &workspace, &query, &[]).expect_err("empty"),
            ResolveError::EmptyScope
        );
    }

    #[test]
    fn non_file_entries_fail() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-special-");
        let workspace = scratch.path().to_path_buf();
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixListener;
            let path = workspace.join("sock");
            let _listener = UnixListener::bind(&path).expect("bind socket");
            let query = NeverQuery;
            assert_eq!(
                resolve(&strings(&["sock"]), &workspace, &query, &[]).expect_err("socket"),
                ResolveError::NotFileOrDir {
                    scope: "sock".to_owned(),
                }
            );
        }
        #[cfg(not(unix))]
        {
            write(&workspace, "pkg/BUILD.bazel", "");
            write(&workspace, "pkg/regular.py", "x = 1\n");
            let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:regular.py\n")]);
            let resolved = resolve(&strings(&["pkg/regular.py"]), &workspace, &query, &[])
                .expect("regular file resolves");
            assert!(
                resolved.targets.contains(&"//pkg:regular.py".to_owned()),
                "regular file owner: {:?}",
                resolved.targets
            );
        }
    }

    #[test]
    fn control_characters_in_names_fail_before_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-control-");
        let workspace = scratch.path().to_path_buf();
        assert_eq!(
            std::fs::read_dir(&workspace)
                .expect("read workspace")
                .count(),
            0,
            "no scope below names a real file, so the rejection never needed one"
        );
        let query = FakeQuery::new(Vec::new());
        for scope in [
            "a\nb.py",
            "a\u{1}b.py",
            "pkg/a\rb.py",
            "pkg/del\u{7f}.py",
            "pkg/dir\u{1}t",
        ] {
            assert_eq!(
                resolve(&strings(&[scope]), &workspace, &query, &[])
                    .expect_err("control character"),
                ResolveError::UnsupportedName {
                    scope: scope.to_owned(),
                },
                "{scope:?}"
            );
        }
        assert!(
            query.calls().is_empty(),
            "invalid names never reach a Bazel query"
        );
    }

    #[test]
    fn unicode_and_space_names_still_resolve() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-unicode-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/naïve file.py", "x = 1\n");
        write(&workspace, "src/ünïcode dir/nested.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n")]);
        let got =
            resolve(&strings(&["pkg/naïve file.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(
            got.targets,
            strings(&["//pkg:lib"]),
            "unicode and spaces stay inside the label"
        );
        let query = NeverQuery;
        let got =
            resolve(&strings(&["src/ünïcode dir"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got.targets, strings(&["//src/ünïcode dir/..."]));
    }

    #[test]
    fn ownerless_files_suggest_explicit_labels() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-no-owner-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err =
            resolve(&strings(&["pkg/orphan.py"]), &workspace, &query, &[]).expect_err("orphan");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                files: strings(&["pkg/orphan.py"]),
                labels: strings(&["//pkg:orphan.py"]),
            }
        );
        assert!(err.to_string().contains("explicit target label"));
    }

    #[test]
    fn file_labels_use_the_nearest_enclosing_package() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-pkg-labels-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/src/deep/b.py", "x = 1\n");
        write(&workspace, "BUILD.bazel", "");
        write(&workspace, "top.py", "x = 1\n");
        assert_eq!(
            file_label(&workspace, "pkg/a.py", "pkg/a.py").expect("label"),
            "//pkg:a.py"
        );
        assert_eq!(
            file_label(&workspace, "pkg/src/deep/b.py", "pkg/src/deep/b.py").expect("label"),
            "//pkg:src/deep/b.py"
        );
        assert_eq!(
            file_label(&workspace, "top.py", "top.py").expect("label"),
            "//:top.py"
        );
        assert_eq!(dir_pattern(""), "//...");
        assert_eq!(dir_pattern("src"), "//src/...");
        assert_eq!(normalize_rel("./pkg/./a.py").expect("dots"), "pkg/a.py");
        assert_eq!(normalize_rel("pkg//a.py").expect("doubles"), "pkg/a.py");
    }

    #[test]
    fn bare_build_marker_and_nearest_package_win() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-pkg-markers-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "legacy/BUILD", "");
        write(&workspace, "legacy/a.py", "x = 1\n");
        write(&workspace, "outer/BUILD.bazel", "");
        write(&workspace, "outer/inner/BUILD.bazel", "");
        write(&workspace, "outer/inner/a.py", "x = 1\n");
        write(&workspace, "outer/loose.py", "x = 1\n");
        assert_eq!(
            file_label(&workspace, "legacy/a.py", "legacy/a.py").expect("label"),
            "//legacy:a.py"
        );
        assert_eq!(
            file_label(&workspace, "outer/inner/a.py", "outer/inner/a.py").expect("label"),
            "//outer/inner:a.py"
        );
        assert_eq!(
            file_label(&workspace, "outer/loose.py", "outer/loose.py").expect("label"),
            "//outer:loose.py"
        );
    }

    #[test]
    fn files_without_any_enclosing_package_fail() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-no-package-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "docs/guide.md", "# guide\n");
        write(&workspace, "other/BUILD.bazel", "");
        let err = resolve(&strings(&["docs/guide.md"]), &workspace, &NeverQuery, &[])
            .expect_err("no package");
        assert_eq!(
            err,
            ResolveError::NotAPackage {
                scope: "docs/guide.md".to_owned(),
            }
        );
        assert!(err.to_string().contains("not a package"), "{err}");
    }

    #[test]
    fn symlinks_are_neither_file_nor_dir() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-symlink-kind-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "target.txt", "x\n");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(workspace.join("target.txt"), workspace.join("link"))
            .expect("link");
        #[cfg(not(windows))]
        std::os::unix::fs::symlink(workspace.join("target.txt"), workspace.join("link"))
            .expect("link");
        {
            let query = NeverQuery;
            assert!(matches!(
                resolve(&strings(&["link"]), &workspace, &query, &[]).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
            assert!(matches!(
                resolve_for_test(&strings(&["link"]), &workspace, &query, &[]).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
            assert!(matches!(
                resolve_run(&strings(&["link"]), &workspace, &query, &[]).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
        }
    }
}
