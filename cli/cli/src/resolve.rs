pub mod classify;
pub mod codegen_expand;
pub mod entry;
pub mod packages;
pub mod query;
pub mod run_deploy;
pub mod test_map;
pub mod types;

pub(crate) use classify::{classify_scopes, first_line, parse_owners, resolve_file_owners};
pub use codegen_expand::expand_codegen_roots;
pub use entry::{resolve, resolve_for_test};
pub(crate) use packages::PackageCache;
pub(crate) use query::{ownership_set_expression, quote_set, run_label_query};
pub use run_deploy::{check_deployable, resolve_deploy, resolve_run, DeployInfo};
pub use test_map::map_owners_to_tests;
#[cfg(test)]
pub(crate) use types::NeverQuery;
pub use types::{ProcessQueryRunner, QueryResult, QueryRunner, ResolveError, ResolvedScope};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;
    use std::io;
    use std::path::Path;

    fn write(workspace: &Path, rel: &str, text: &str) {
        let full = workspace.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write file");
    }

    #[test]
    fn resolve_errors_render_actionable_messages() {
        let cases = [
            (ResolveError::EmptyScope, "empty scope"),
            (
                ResolveError::RelativeLabel {
                    scope: ":c".to_owned(),
                },
                ":c",
            ),
            (
                ResolveError::ExternalScope {
                    scope: "@r//p".to_owned(),
                },
                "@r//p",
            ),
            (
                ResolveError::OutsideWorkspace {
                    scope: "../x".to_owned(),
                },
                "../x",
            ),
            (
                ResolveError::PathNotFound {
                    scope: "n.py".to_owned(),
                },
                "n.py",
            ),
            (
                ResolveError::NotFileOrDir {
                    scope: "s".to_owned(),
                },
                "unsupported path",
            ),
            (
                ResolveError::NotAPackage {
                    scope: "docs/g.md".to_owned(),
                },
                "not a package",
            ),
            (
                ResolveError::UnsupportedName {
                    scope: "n".to_owned(),
                },
                "control characters",
            ),
            (
                ResolveError::NoOwner {
                    file: "f.py".to_owned(),
                    label: "//:f.py".to_owned(),
                },
                "f.py",
            ),
            (
                ResolveError::QueryFailed {
                    label: "//:f.py".to_owned(),
                    detail: "boom".to_owned(),
                },
                "boom",
            ),
        ];
        for (error, needle) in cases {
            assert!(error.to_string().contains(needle), "{error:?}");
        }
    }

    struct FailIo;

    impl QueryRunner for FailIo {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Err(io::Error::other("boom"))
        }
    }

    #[test]
    fn query_io_errors_become_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-query-io-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &FailIo).expect_err("io");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
        let err = resolve_run(&strings(&["pkg/a.py"]), &workspace, &FailIo).expect_err("io");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
        let err =
            map_owners_to_tests(&strings(&["//pkg:lib"]), &workspace, &FailIo).expect_err("io");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
    }

    #[test]
    fn not_a_directory_maps_to_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-not-a-dir-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg", "file, not dir\n");
        let query = NeverQuery;
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &query).expect_err("enotdir");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
        let err =
            resolve_for_test(&strings(&["pkg/a.py"]), &workspace, &query).expect_err("enotdir");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
        let err = resolve_run(&strings(&["pkg/a.py"]), &workspace, &query).expect_err("enotdir");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
    }

    #[test]
    fn test_scope_rejects_external_and_relative() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-reject-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        assert_eq!(
            resolve_for_test(&strings(&["@r//p"]), &workspace, &query).expect_err("ext"),
            ResolveError::ExternalScope {
                scope: "@r//p".to_owned(),
            }
        );
        assert_eq!(
            resolve_for_test(&strings(&[":c"]), &workspace, &query).expect_err("rel"),
            ResolveError::RelativeLabel {
                scope: ":c".to_owned(),
            }
        );
        assert_eq!(
            resolve_run(&strings(&["@r//p"]), &workspace, &query).expect_err("ext"),
            ResolveError::ExternalScope {
                scope: "@r//p".to_owned(),
            }
        );
        assert_eq!(
            resolve_run(&strings(&[":c"]), &workspace, &query).expect_err("rel"),
            ResolveError::RelativeLabel {
                scope: ":c".to_owned(),
            }
        );
    }

    #[test]
    fn test_scope_missing_files_fail() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-scope-missing-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        assert_eq!(
            resolve_for_test(&strings(&["nope.py"]), &workspace, &query).expect_err("missing"),
            ResolveError::PathNotFound {
                scope: "nope.py".to_owned(),
            }
        );
        assert_eq!(
            resolve_run(&strings(&["nope.py"]), &workspace, &query).expect_err("missing"),
            ResolveError::PathNotFound {
                scope: "nope.py".to_owned(),
            }
        );
    }

    #[test]
    fn control_chars_in_names_are_rejected() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-control-names-");
        let workspace = scratch.path().to_path_buf();
        let dir = "app\x01";
        let file = "app\x01/main.py";
        std::fs::create_dir_all(workspace.join(dir)).expect("dir");
        write(&workspace, file, "x = 1\n");
        write(&workspace, "app\x01/BUILD.bazel", "");
        let query = NeverQuery;
        assert!(matches!(
            resolve_run(&strings(&[dir]), &workspace, &query).expect_err("dir"),
            ResolveError::UnsupportedName { .. }
        ));
        assert!(matches!(
            resolve_run(&strings(&[file]), &workspace, &query).expect_err("file"),
            ResolveError::UnsupportedName { .. }
        ));
    }
}
