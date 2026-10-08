use std::path::Path;

use super::{run_label_query, QueryRunner, ResolveError};

pub fn expand_codegen_roots(
    label: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup: &[String],
) -> Result<Vec<String>, ResolveError> {
    let expression = dx_codegen::expansion_expression(label);
    let projections = run_label_query(&expression, workspace, runner, startup)?;
    Ok(dx_codegen::expand_roots(label, &projections))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::QueryResult;
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

    #[test]
    fn expansion_queries_shard_rdeps_and_unions_schema() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(
            "//generation:codegen_prost_fixture\n//generation:codegen_prost_fixture\n",
        )]);
        let got =
            expand_codegen_roots("//generation:result_proto", &workspace, &query, &[]).expect("expand");
        assert_eq!(
            got,
            vec![
                "//generation:codegen_prost_fixture".to_owned(),
                "//generation:result_proto".to_owned(),
            ]
        );
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, workspace, "expansion runs in the workspace");
        assert_eq!(
            calls[0].0.last().expect("expression"),
            "kind('.*codegen_shard rule', rdeps(//..., set(\"//generation:result_proto\")))"
        );
    }

    #[test]
    fn empty_projections_keep_the_single_label() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-empty-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let got =
            expand_codegen_roots("//generation:result_proto", &workspace, &query, &[]).expect("expand");
        assert_eq!(got, vec!["//generation:result_proto".to_owned()]);
    }

    #[test]
    fn expansion_failures_report_the_first_bazel_line() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-fail-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::failed("\n  query failed: blah  \nmore\n")]);
        let err = expand_codegen_roots("//generation:result_proto", &workspace, &query, &[])
            .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label:
                    "kind('.*codegen_shard rule', rdeps(//..., set(\"//generation:result_proto\")))"
                        .to_owned(),
                detail: "query failed: blah".to_owned(),
            }
        );
    }
}
