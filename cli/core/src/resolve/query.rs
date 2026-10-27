use std::path::Path;

use super::{parse_owners, QueryFailure, QueryRunner, ResolveError};

pub fn quote_set(items: &[String]) -> String {
    let mut sorted: Vec<&String> = items.iter().collect();
    sorted.sort();
    sorted
        .iter()
        .map(|item| quote_label(item))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn quote_label(label: &str) -> String {
    dx_codegen::quote_label(label)
}

fn query_argv(expression: &str, startup_options: &[String]) -> Vec<String> {
    let mut argv = dx_process::startup_argv(startup_options);
    argv.push("query".to_owned());
    argv.push("--".to_owned());
    argv.push(expression.to_owned());
    argv
}

pub(crate) fn run_label_query(
    expression: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let argv = query_argv(expression, startup_options);
    let result = ResolveError::run(runner, &argv, workspace, expression)?;
    parse_owners(&result.stdout, expression)
}

pub fn run_label_query_file(
    expression: &str,
    label: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let mut file = tempfile::NamedTempFile::new().map_err(|error| {
        ResolveError::query(
            label,
            QueryFailure::Prepare {
                detail: format!("cannot write the query file: {error}"),
            },
        )
    })?;
    std::io::Write::write_all(&mut file, expression.as_bytes()).map_err(|error| {
        ResolveError::query(
            label,
            QueryFailure::Prepare {
                detail: format!("cannot write the query file: {error}"),
            },
        )
    })?;
    let mut argv = dx_process::startup_argv(startup_options);
    argv.push("query".to_owned());
    argv.push(format!("--query_file={}", file.path().display()));
    let result = ResolveError::run(runner, &argv, workspace, label)?;
    parse_owners(&result.stdout, label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{first_line, resolve, QueryFailure, QueryResult};
    use dx_testing::cli::strings;
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
    fn query_failures_keep_the_bazel_status_and_a_bounded_excerpt() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-query-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let label = "kind('rule', //...)";
        let query = FakeQuery::new(vec![QueryResult {
            code: Some(7),
            stdout: b"//pkg:partial\n".to_vec(),
            stderr: b"Loading: 0 packages loaded\nERROR: no such package 'pkg'\nERROR: Evaluation of query failed\n".to_vec(),
        }]);
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: label.to_owned(),
                failure: QueryFailure::exit_for_test(
                    7,
                    "Loading: 0 packages loaded\nERROR: no such package 'pkg'\nERROR: Evaluation of query failed"
                ),
            }
        );
        assert_eq!(
            err.to_string(),
            format!("query {label} failed: Bazel exited 7")
        );
        let query = FakeQuery::new(vec![QueryResult {
            code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }]);
        let err = resolve(&strings(&["pkg/a.py"]), &workspace, &query, &[]).expect_err("signal");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: label.to_owned(),
                failure: QueryFailure::Exit {
                    status: None,
                    excerpt: None,
                },
            }
        );
        assert!(
            err.to_string().ends_with(
                "Bazel terminated by signal; cause unknown: Bazel printed no diagnostic"
            ),
            "{err}"
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
                label: label.to_owned(),
                failure: QueryFailure::Output {
                    detail: "query output is not UTF-8".to_owned(),
                },
            }
        );
    }

    #[test]
    fn a_query_that_cannot_launch_is_typed_as_a_launch_failure() {
        struct Missing;
        impl QueryRunner for Missing {
            fn run_query(&self, _argv: &[String], _cwd: &Path) -> std::io::Result<QueryResult> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "bazel: not found",
                ))
            }
        }
        let scratch = dx_test_scratch::scratch("dx-resolve-test-query-launch-");
        let workspace = scratch.path().to_path_buf();
        let err = run_label_query("owners", &workspace, &Missing, &[]).expect_err("launch");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "owners".to_owned(),
                failure: QueryFailure::Launch {
                    detail: "bazel: not found".to_owned(),
                },
            }
        );
        assert_eq!(
            err.to_string(),
            "query owners failed: could not launch Bazel: bazel: not found"
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
