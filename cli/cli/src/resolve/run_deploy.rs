use std::path::Path;

use super::classify::classify_scopes;
use super::packages::PackageCache;
use super::{
    first_line, ownership_set_expression, quote_set, run_label_query, QueryRunner, ResolveError,
};

fn runnable_set_expression(labels: &[String]) -> String {
    format!(
        "kind('.*_binary rule', rdeps(//..., set({}), 1))",
        quote_set(labels)
    )
}

fn dir_runnable_expression(pattern: &str) -> String {
    format!("kind('.*_binary rule', {pattern})")
}

fn is_run_pattern(label: &str) -> bool {
    label.contains("...") || label.contains('*')
}

pub fn resolve_run(
    scopes: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    if scopes.is_empty() {
        return Err(ResolveError::EmptyScope);
    }
    let mut cache = PackageCache::default();
    let classified = classify_scopes(scopes, workspace, &mut cache)?;
    if classified.files.is_empty() && classified.patterns.is_empty() {
        if !classified.labels.iter().any(|label| is_run_pattern(label)) {
            return Ok(classified.labels);
        }
        let mut targets: Vec<String> = Vec::new();
        for label in &classified.labels {
            if is_run_pattern(label) {
                targets.extend(run_label_query(
                    &dir_runnable_expression(label),
                    workspace,
                    runner,
                    startup_options,
                )?);
            } else {
                targets.push(label.clone());
            }
        }
        let mut seen = std::collections::HashSet::new();
        targets.retain(|target| seen.insert(target.clone()));
        if targets.is_empty() {
            return Err(ResolveError::NoRunnable {
                scopes: scopes.to_vec(),
            });
        }
        return Ok(targets);
    }
    let mut candidates: Vec<String> = classified.labels;
    if !classified.files.is_empty() {
        let labels: Vec<String> = classified
            .files
            .iter()
            .map(|file| file.label.clone())
            .collect();
        let found = run_label_query(
            &runnable_set_expression(&labels),
            workspace,
            runner,
            startup_options,
        )?;
        if found.is_empty() {
            let owned = run_label_query(
                &ownership_set_expression(&labels),
                workspace,
                runner,
                startup_options,
            )?;
            if owned.is_empty() {
                let first = &classified.files[0];
                return Err(ResolveError::NoOwner {
                    file: first.scope.clone(),
                    label: first.label.clone(),
                });
            }
        } else {
            candidates.extend(found);
        }
    }
    for pattern in &classified.patterns {
        let found = run_label_query(
            &dir_runnable_expression(pattern),
            workspace,
            runner,
            startup_options,
        )?;
        candidates.extend(found);
    }
    candidates.sort();
    candidates.dedup();
    if candidates.is_empty() {
        let mut paths = classified.paths;
        paths.sort();
        paths.dedup();
        return Err(ResolveError::NoRunnable { scopes: paths });
    }
    if candidates.len() > 1 {
        return Err(ResolveError::AmbiguousRunnable { candidates });
    }
    Ok(candidates)
}

pub fn resolve_deploy(scopes: &[String]) -> Result<String, ResolveError> {
    if scopes.len() != 1 {
        return Err(ResolveError::DeployCount {
            count: scopes.len(),
        });
    }
    let scope = &scopes[0];
    if scope.starts_with("//") {
        if scope.contains("...") {
            return Err(ResolveError::DeployScope {
                scope: scope.clone(),
            });
        }
        return Ok(scope.clone());
    }
    Err(ResolveError::DeployScope {
        scope: scope.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployInfo {
    pub label: String,
    pub has_provider: bool,
    pub profile_raw: String,
    pub app_raw: String,
    pub executable: bool,
}

fn deploy_starlark_expr() -> String {
    const PROVIDER: &str = "//deploy/rules:defs.bzl%DxDeployInfo";
    format!(
        "(str(providers(target).get('{PROVIDER}', None) != None)) + '|' + \
         ((str(providers(target).get('{PROVIDER}', None).profile) if providers(target).get('{PROVIDER}', None) != None else 'NONE')) + '|' + \
         ((str(providers(target).get('{PROVIDER}', None).app) if providers(target).get('{PROVIDER}', None) != None else 'NONE')) + '|' + \
         str(target.files_to_run.executable != None)"
    )
}

fn deploy_query_argv(label: &str, startup_options: &[String]) -> Vec<String> {
    let mut argv = dx_process::startup_argv(startup_options);
    argv.push("cquery".to_owned());
    argv.push(label.to_owned());
    argv.push("--output=starlark".to_owned());
    argv.push(format!("--starlark:expr={}", deploy_starlark_expr()));
    argv
}

pub fn check_deployable(
    label: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<DeployInfo, ResolveError> {
    let argv = deploy_query_argv(label, startup_options);
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| ResolveError::QueryFailed {
            label: label.to_owned(),
            detail: error.to_string(),
        })?;
    if result.code != Some(0) {
        return Err(ResolveError::QueryFailed {
            label: label.to_owned(),
            detail: first_line(&result.stderr),
        });
    }
    let text = std::str::from_utf8(&result.stdout).map_err(|_| ResolveError::QueryFailed {
        label: label.to_owned(),
        detail: "query output is not UTF-8".to_owned(),
    })?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| ResolveError::QueryFailed {
            label: label.to_owned(),
            detail: "cquery returned no deploy observation".to_owned(),
        })?;
    let mut parts = line.split('|');
    let (has, profile_raw, app_raw, exe) = match (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) {
        (Some(has), Some(profile), Some(app), Some(exe), None) => (has, profile, app, exe),
        _ => {
            return Err(ResolveError::QueryFailed {
                label: label.to_owned(),
                detail: "cquery returned a malformed deploy observation".to_owned(),
            });
        }
    };
    let has_provider = match has {
        "True" => true,
        "False" => false,
        _ => {
            return Err(ResolveError::QueryFailed {
                label: label.to_owned(),
                detail: "cquery returned a malformed deploy observation".to_owned(),
            });
        }
    };
    let executable = match exe {
        "True" => true,
        "False" => false,
        _ => {
            return Err(ResolveError::QueryFailed {
                label: label.to_owned(),
                detail: "cquery returned a malformed deploy observation".to_owned(),
            });
        }
    };
    if !has_provider && !executable {
        return Err(ResolveError::NotDeployable {
            label: label.to_owned(),
        });
    }
    Ok(DeployInfo {
        label: label.to_owned(),
        has_provider,
        profile_raw: profile_raw.to_owned(),
        app_raw: app_raw.to_owned(),
        executable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::NeverQuery;
    use crate::resolve::QueryResult;
    use crate::test_support::strings;
    use std::cell::RefCell;
    use std::io;
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

        fn failed(stderr: &str) -> QueryResult {
            QueryResult {
                code: Some(2),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
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

    #[test]
    fn run_labels_pass_through_without_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-labels-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        let got = resolve_run(&strings(&["//app:bin"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got, strings(&["//app:bin"]));
    }

    #[test]
    fn run_empty_scope_is_a_usage_error() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-empty-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        let err = resolve_run(&[], &workspace, &query, &[]).expect_err("empty");
        assert_eq!(err, ResolveError::EmptyScope);
    }

    #[test]
    fn run_file_resolves_single_binary_owner() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-file-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "app/BUILD.bazel", "");
        write(&workspace, "app/main.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//app:bin\n")]);
        let got =
            resolve_run(&strings(&["app/main.py"]), &workspace, &query, &[]).expect("resolve");
        assert_eq!(got, strings(&["//app:bin"]));
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('.*_binary rule', rdeps(//..., set(\"//app:main.py\"), 1))"
        );
    }

    #[test]
    fn run_file_without_binary_reports_no_runnable() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-no-bin-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n"), FakeQuery::ok("//pkg:lib\n")]);
        let err =
            resolve_run(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("no runnable");
        assert_eq!(
            err,
            ResolveError::NoRunnable {
                scopes: strings(&["pkg/a.py"]),
            }
        );
        assert!(err.to_string().contains("no executable target"), "{err}");
    }

    #[test]
    fn run_file_without_any_owner_reports_no_owner() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-no-owner-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n"), FakeQuery::ok("\n")]);
        let err =
            resolve_run(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("no owner");
        assert_eq!(
            err,
            ResolveError::NoOwner {
                file: "pkg/a.py".to_owned(),
                label: "//pkg:a.py".to_owned(),
            }
        );
    }

    #[test]
    fn run_dir_with_two_binaries_reports_ambiguous_candidates() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-ambiguous-");
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("app")).expect("dir");
        let query = FakeQuery::new(vec![FakeQuery::ok("//app:two\n//app:one\n")]);
        let err = resolve_run(&strings(&["app"]), &workspace, &query, &[]).expect_err("ambiguous");
        assert_eq!(
            err,
            ResolveError::AmbiguousRunnable {
                candidates: strings(&["//app:one", "//app:two"]),
            }
        );
        assert!(
            err.to_string().contains("multiple executable targets"),
            "{err}"
        );
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('.*_binary rule', //app/...)"
        );
    }

    #[test]
    fn runnable_query_failures_report_first_line() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-query-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "app/BUILD.bazel", "");
        write(&workspace, "app/main.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::failed("nope\n")]);
        let err =
            resolve_run(&strings(&["app/main.py"]), &workspace, &query, &[]).expect_err("fail");
        assert!(matches!(err, ResolveError::QueryFailed { .. }), "{err:?}");
    }

    #[test]
    fn run_mixed_label_and_file_skips_label_in_second_pass() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-run-mixed-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "app/BUILD.bazel", "");
        write(&workspace, "app/main.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//app:bin\n")]);
        let got = resolve_run(
            &strings(&["//app:bin", "app/main.py"]),
            &workspace,
            &query,
            &[],
        )
        .expect("mixed");
        assert_eq!(got, strings(&["//app:bin"]));
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn run_cross_pattern_order_is_input_order_with_sorted_batches() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-cross-pattern-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//b:two\n//b:one\n"),
            FakeQuery::ok("//a:two\n//a:one\n"),
        ]);
        let got = resolve_run(&strings(&["//b/...", "//a/..."]), &workspace, &query, &[])
            .expect("cross-pattern");
        assert_eq!(
            got,
            strings(&["//b:one", "//b:two", "//a:one", "//a:two"]),
            "batches stay in input order, each sorted"
        );
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-dedup-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok("//a:bin\n//a:other\n")]);
        let got =
            resolve_run(&strings(&["//a:bin", "//a/..."]), &workspace, &query, &[]).expect("dedup");
        assert_eq!(got, strings(&["//a:bin", "//a:other"]));
    }

    #[test]
    fn run_file_alias_is_fail_closed_with_explicit_label_hint() {
        let scratch = dx_test_scratch::scratch("dx-resolve-run-test-alias-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "app/BUILD.bazel", "");
        write(&workspace, "app/main.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n"), FakeQuery::ok("//app:lib\n")]);
        let err =
            resolve_run(&strings(&["app/main.py"]), &workspace, &query, &[]).expect_err("alias");
        assert_eq!(
            err,
            ResolveError::NoRunnable {
                scopes: strings(&["app/main.py"]),
            }
        );
        assert!(
            err.to_string().contains("explicit runnable label"),
            "alias fail-closed must hint explicit label: {err}"
        );
    }
}
