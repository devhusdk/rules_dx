use std::io;
use std::path::{Component, Path};

use super::packages::PackageCache;
use super::{ownership_set_expression, run_label_query, QueryRunner, ResolveError};

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
        let entry = workspace.join(&rel);
        let metadata = std::fs::symlink_metadata(&entry).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
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
            if rel.chars().any(char::is_control) {
                return Err(ResolveError::UnsupportedName { scope: raw.clone() });
            }
            classified.patterns.push(dir_pattern(&rel));
            classified.paths.push(raw.clone());
        } else if metadata.is_file() {
            if rel.chars().any(char::is_control) {
                return Err(ResolveError::UnsupportedName { scope: raw.clone() });
            }
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

pub(crate) fn resolve_file_owners(
    files: &[FileScope],
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Result<Vec<String>, ResolveError> {
    let labels: Vec<String> = files.iter().map(|file| file.label.clone()).collect();
    let expression = ownership_set_expression(&labels);
    let owners = run_label_query(&expression, workspace, runner)?;
    if owners.is_empty() {
        let first = &files[0];
        return Err(ResolveError::NoOwner {
            file: first.scope.clone(),
            label: first.label.clone(),
        });
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
        let got = resolve(&[], &workspace, &query).expect("resolve");
        assert_eq!(got.scope, Scope::Repository);
        assert_eq!(got.targets, strings(&["//..."]));
    }

    #[test]
    fn label_only_scopes_pass_through_in_order() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-labels-");
        let workspace = scratch.path().to_path_buf();
        let input = strings(&["//b/...", "//a:one", "@repo//c/..."]);
        let err = resolve(&input, &workspace, &query).expect_err("external must fail");
        assert_eq!(
            err,
            ResolveError::ExternalScope {
                scope: "@repo//c/...".to_owned(),
            }
        );
        let input = strings(&["//b/...", "//a:one"]);
        let got = resolve(&input, &workspace, &query).expect("resolve");
        assert_eq!(got.scope, Scope::Labels(strings(&["//b/...", "//a:one"])));
        assert_eq!(got.targets, strings(&["//b/...", "//a:one"]));
    }

    #[test]
    fn relative_labels_fail_with_guidance() {
        let query = NeverQuery;
        let scratch = dx_test_scratch::scratch("dx-resolve-test-relative-");
        let workspace = scratch.path().to_path_buf();
        let err = resolve(&strings(&[":corpus"]), &workspace, &query).expect_err("relative");
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
        let got = resolve(&strings(&["pkg/a.py"]), &workspace, &query).expect("resolve");
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
        resolve(&strings(&["pkg/my file.py"]), &workspace, &query).expect("resolve");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('rule', rdeps(//..., set(\"//pkg:my file.py\"), 1))"
        );
        assert_eq!(quote_label("//pkg:a\"b\\c"), "\"//pkg:a\\\"b\\\\c\"");
    }

    #[test]
    fn multiple_files_share_one_bounded_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n//pkg:extra\n")]);
        let got =
            resolve(&strings(&["pkg/b.py", "pkg/a.py"]), &workspace, &query).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:extra", "//pkg:lib"]));
        let calls = query.calls();
        assert_eq!(calls.len(), 1, "one bounded query per resolver call");
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('rule', rdeps(//..., set(\"//pkg:a.py\" \"//pkg:b.py\"), 1))"
        );
    }

    #[test]
    fn empty_batch_mapping_names_the_first_file() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-batch-empty-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err =
            resolve(&strings(&["pkg/a.py", "pkg/b.py"]), &workspace, &query).expect_err("orphans");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                file: "pkg/a.py".to_owned(),
                label: "//pkg:a.py".to_owned(),
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
        let err = resolve(&strings(&["pkg/a.py", "docs/guide.md"]), &workspace, &query)
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
        let got = resolve(&strings(&["top.py"]), &workspace, &query).expect("resolve");
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
        let got = resolve(&strings(&["pkg/a.py"]), &workspace, &query).expect("resolve");
        assert_eq!(got.targets, strings(&["//pkg:real"]));
    }

    #[test]
    fn directory_becomes_recursive_pattern_without_query_or_listing() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-dir-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "src/nested/deep.py", "x = 1\n");
        write(&workspace, "src/top.py", "x = 1\n");
        let query = NeverQuery;
        let got = resolve(&strings(&["src"]), &workspace, &query).expect("resolve");
        assert_eq!(got.targets, strings(&["//src/..."]));
        assert_eq!(got.scope, Scope::ResolvedOwners(strings(&["//src/..."])));
    }

    #[test]
    fn workspace_root_directory_maps_to_repository_pattern() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-root-dir-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        for root in [".", "./"] {
            let got = resolve(&strings(&[root]), &workspace, &query).expect("resolve");
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
        let got = resolve(&strings(&["//z:z", "pkg/a.py"]), &workspace, &query).expect("resolve");
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
            resolve(&strings(&["nope.py"]), &workspace, &query).expect_err("missing"),
            ResolveError::PathNotFound {
                scope: "nope.py".to_owned(),
            }
        );
        for escaping in ["/abs/path.py", "../escape.py", "pkg/../../escape.py"] {
            assert_eq!(
                resolve(&strings(&[escaping]), &workspace, &query).expect_err("escape"),
                ResolveError::OutsideWorkspace {
                    scope: escaping.to_owned(),
                }
            );
        }
        assert_eq!(
            resolve(&strings(&[""]), &workspace, &query).expect_err("empty"),
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
                resolve(&strings(&["sock"]), &workspace, &query).expect_err("socket"),
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
            let resolved = resolve(&strings(&["pkg/regular.py"]), &workspace, &query)
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
        write(&workspace, "a\nb.py", "x = 1\n");
        let query = NeverQuery;
        assert_eq!(
            resolve(&strings(&["a\nb.py"]), &workspace, &query).expect_err("control"),
            ResolveError::UnsupportedName {
                scope: "a\nb.py".to_owned(),
            }
        );
    }

    #[test]
    fn ownerless_files_suggest_explicit_labels() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-no-owner-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let err = resolve(&strings(&["pkg/orphan.py"]), &workspace, &query).expect_err("orphan");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                file: "pkg/orphan.py".to_owned(),
                label: "//pkg:orphan.py".to_owned(),
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
        let err =
            resolve(&strings(&["docs/guide.md"]), &workspace, &NeverQuery).expect_err("no package");
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
                resolve(&strings(&["link"]), &workspace, &query).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
            assert!(matches!(
                resolve_for_test(&strings(&["link"]), &workspace, &query).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
            assert!(matches!(
                resolve_run(&strings(&["link"]), &workspace, &query).expect_err("link"),
                ResolveError::NotFileOrDir { .. }
            ));
        }
    }
}
