use std::path::Path;

use dx_process::{launcher_argv0, WORKFLOW_STARTUP_OPTS};

use super::{first_line, parse_owners, QueryRunner, ResolveError};

pub(crate) fn quote_set(items: &[String]) -> String {
    let mut sorted: Vec<&String> = items.iter().collect();
    sorted.sort();
    sorted
        .iter()
        .map(|item| quote_label(item))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn ownership_set_expression(labels: &[String]) -> String {
    format!("kind('rule', rdeps(//..., set({}), 1))", quote_set(labels))
}

pub(crate) fn quote_label(label: &str) -> String {
    dx_codegen::quote_label(label)
}

fn query_argv(expression: &str, startup: &[String]) -> Vec<String> {
    let mut argv = Vec::with_capacity(WORKFLOW_STARTUP_OPTS.len() + startup.len() + 4);
    argv.push(launcher_argv0().to_owned());
    argv.extend(WORKFLOW_STARTUP_OPTS.iter().map(ToString::to_string));
    argv.extend(startup.iter().cloned());
    argv.push("query".to_owned());
    argv.push("--".to_owned());
    argv.push(expression.to_owned());
    argv
}

pub(crate) fn run_label_query(
    expression: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup: &[String],
) -> Result<Vec<String>, ResolveError> {
    let argv = query_argv(expression, startup);
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| ResolveError::QueryFailed {
            label: expression.to_owned(),
            detail: error.to_string(),
        })?;
    if result.code != Some(0) {
        return Err(ResolveError::QueryFailed {
            label: expression.to_owned(),
            detail: first_line(&result.stderr),
        });
    }
    parse_owners(&result.stdout, expression)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, QueryResult};
    use crate::test_support::strings;
    use std::cell::RefCell;

    struct FakeQuery {
        outputs: RefCell<Vec<QueryResult>>,
    }

    impl FakeQuery {
        fn new(outputs: Vec<QueryResult>) -> Self {
            FakeQuery {
                outputs: RefCell::new(outputs),
            }
        }

        fn failed(stderr: &str) -> QueryResult {
            QueryResult {
                code: Some(2),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            }
        }
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
            let _ = (argv, cwd);
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }
    fn write(workspace: &Path, rel: &str, text: &str) {
        let full = workspace.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write file");
    }

    #[test]
    fn randomized_query_order_yields_identical_argv() {
        let forward = strings(&["//pkg:b.py", "//pkg:a.py", "//pkg:c.py"]);
        let reversed = strings(&["//pkg:c.py", "//pkg:a.py", "//pkg:b.py"]);
        let rotated = strings(&["//pkg:a.py", "//pkg:c.py", "//pkg:b.py"]);
        assert_eq!(quote_set(&forward), quote_set(&reversed));
        assert_eq!(quote_set(&forward), quote_set(&rotated));
        assert_eq!(
            ownership_set_expression(&forward),
            ownership_set_expression(&reversed)
        );
        assert_eq!(
            ownership_set_expression(&forward),
            ownership_set_expression(&rotated)
        );
        let shuffled_outputs = [
            "//pkg:a\n//pkg:b\n//pkg:c\n",
            "//pkg:c\n//pkg:a\n//pkg:b\n",
            "//pkg:b\n//pkg:c\n//pkg:a\n//pkg:b\n",
        ];
        let mut parsed = Vec::new();
        for stdout in shuffled_outputs {
            let owners = parse_owners(stdout.as_bytes(), "owners").expect("parse owners");
            assert_eq!(owners, vec!["//pkg:a", "//pkg:b", "//pkg:c"]);
            parsed.push(owners);
        }
        assert_eq!(parsed[0], parsed[1]);
        assert_eq!(parsed[0], parsed[2]);
        struct Capturing {
            output: QueryResult,
            seen: RefCell<Vec<Vec<String>>>,
        }
        impl QueryRunner for Capturing {
            fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
                self.seen.borrow_mut().push(argv.to_vec());
                let _ = cwd;
                Ok(QueryResult {
                    code: self.output.code,
                    stdout: self.output.stdout.clone(),
                    stderr: self.output.stderr.clone(),
                })
            }
        }
        let workspace = dx_test_scratch::scratch("dx-resolve-test-query-order-")
            .path()
            .to_path_buf();
        let mut argvs = Vec::new();
        for stdout in shuffled_outputs {
            let runner = Capturing {
                output: QueryResult {
                    code: Some(0),
                    stdout: stdout.as_bytes().to_vec(),
                    stderr: Vec::new(),
                },
                seen: RefCell::new(Vec::new()),
            };
            let owners = run_label_query("owners", &workspace, &runner, &[]).expect("query");
            assert_eq!(owners, vec!["//pkg:a", "//pkg:b", "//pkg:c"]);
            assert_eq!(runner.seen.borrow().len(), 1);
            argvs.push(runner.seen.borrow()[0].clone());
        }
        assert_eq!(argvs[0], argvs[1]);
        assert_eq!(argvs[0], argvs[2]);
    }

    #[test]
    fn query_failures_report_the_first_bazel_line() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-query-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::failed(
            "\n  no such package 'pkg': BUILD file not found  \nmore context\n",
        )]);
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "kind('rule', rdeps(//..., set(\"//pkg:a.py\"), 1))".to_owned(),
                detail: "no such package 'pkg': BUILD file not found".to_owned(),
            }
        );
        let query = FakeQuery::new(vec![QueryResult {
            code: Some(0),
            stdout: vec![0xff, 0xfe],
            stderr: Vec::new(),
        }]);
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("non-utf8");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "kind('rule', rdeps(//..., set(\"//pkg:a.py\"), 1))".to_owned(),
                detail: "query output is not UTF-8".to_owned(),
            }
        );
    }

    #[test]
    fn empty_stderr_reports_a_default_diagnostic() {
        assert_eq!(first_line(b""), "no Bazel diagnostic");
        assert_eq!(first_line(b"\n  \n"), "no Bazel diagnostic");
        let long = format!("x: {}", "y".repeat(400));
        let got = first_line(long.as_bytes());
        assert!(got.len() <= 303, "bounded: {}", got.len());
        assert!(got.ends_with("..."));
    }

    #[test]
    fn a_wide_character_on_the_truncation_edge_is_not_split() {
        let mut stderr = "x".repeat(298);
        stderr.push('€');
        stderr.push_str("ERROR: /home/u/pkg/BUILD.bazel");
        let got = first_line(stderr.as_bytes());
        assert_eq!(got, format!("{}...", "x".repeat(298)));
        assert!(got.len() <= 301, "bounded: {}", got.len());
    }
}
