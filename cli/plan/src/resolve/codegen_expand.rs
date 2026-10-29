use std::path::Path;

use super::rule_graph::RuleGraph;
use super::{QueryRunner, ResolveError};

pub fn expand_codegen_roots(
    label: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let graph = RuleGraph::load(workspace, runner, startup_options)?;
    let projections = graph.dependents_of_kind(label, dx_codegen::is_shard_kind);
    Ok(dx_codegen::expand_roots(label, &projections))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{FakeQuery, QueryFailure};
    use dx_testing::cli::{graph_rule_of_kind, graph_stdout};
    use std::path::PathBuf;

    fn assert_no_rdeps(calls: &[(Vec<String>, PathBuf)]) {
        for (argv, _) in calls {
            assert!(!argv.iter().any(|arg| arg.contains("rdeps(")), "{argv:?}");
        }
    }

    #[test]
    fn expansion_reads_the_rule_graph_and_unions_schema() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            graph_rule_of_kind(
                "//generation:result_proto",
                "proto_library",
                &["//generation:result.proto"],
                &[],
            ),
            graph_rule_of_kind(
                "//generation:result_proto_rs",
                "rust_prost_library",
                &["//generation:result_proto"],
                &[],
            ),
            graph_rule_of_kind(
                "//generation:codegen_prost_fixture",
                "prost_codegen_shard",
                &["//generation:result_proto_rs"],
                &[],
            ),
        ]))]);
        let got = expand_codegen_roots("//generation:result_proto", &workspace, &query, &[])
            .expect("expand");
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
            "kind('rule', //...)"
        );
        assert_no_rdeps(&calls);
    }

    #[test]
    fn a_flatbuffers_shard_graph_expands_to_the_shard_plus_the_schema() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-fbs-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            graph_rule_of_kind(
                "//schema:lib",
                "flatbuffers_library",
                &["//schema:a.fbs"],
                &[],
            ),
            graph_rule_of_kind(
                "//schema:lib_cc",
                "flatbuffers_cc_library",
                &["//schema:lib"],
                &[],
            ),
            graph_rule_of_kind(
                "//schema:lib_cc_shard",
                "flatbuffers_codegen_shard",
                &["//schema:lib_cc"],
                &[],
            ),
        ]))]);
        let got = expand_codegen_roots("//schema:lib", &workspace, &query, &[]).expect("expand");
        assert_eq!(
            got,
            vec![
                "//schema:lib".to_owned(),
                "//schema:lib_cc_shard".to_owned()
            ]
        );
    }

    #[test]
    fn a_capnp_shard_graph_expands_to_the_shard_plus_the_schema() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-capnp-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            graph_rule_of_kind("//schema:lib", "capnp_library", &["//schema:a.capnp"], &[]),
            graph_rule_of_kind(
                "//schema:lib_cc",
                "capnp_cc_library",
                &["//schema:lib"],
                &[],
            ),
            graph_rule_of_kind(
                "//schema:lib_cc_shard",
                "capnp_codegen_shard",
                &["//schema:lib_cc"],
                &[],
            ),
        ]))]);
        let got = expand_codegen_roots("//schema:lib", &workspace, &query, &[]).expect("expand");
        assert_eq!(
            got,
            vec![
                "//schema:lib".to_owned(),
                "//schema:lib_cc_shard".to_owned()
            ]
        );
    }

    #[test]
    fn a_flatbuffers_rust_shard_graph_expands_to_the_shard_plus_the_schema() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-fbs-rs-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            graph_rule_of_kind(
                "//schema:lib",
                "flatbuffers_library",
                &["//schema:a.fbs"],
                &[],
            ),
            graph_rule_of_kind(
                "//schema:lib_rs_generated",
                "_flatbuffers_rust_srcs",
                &["//schema:lib"],
                &[],
            ),
            graph_rule_of_kind(
                "//schema:lib_rs_shard",
                "flatbuffers_codegen_shard",
                &["//schema:lib_rs_generated"],
                &[],
            ),
        ]))]);
        let got = expand_codegen_roots("//schema:lib", &workspace, &query, &[]).expect("expand");
        assert_eq!(
            got,
            vec![
                "//schema:lib".to_owned(),
                "//schema:lib_rs_shard".to_owned()
            ]
        );
    }

    #[test]
    fn an_openapi_shard_graph_expands_to_the_shard_plus_the_spec() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-openapi-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            graph_rule_of_kind(
                "//shop:spec",
                "api_spec_library",
                &["//shop:openapi.yaml"],
                &[],
            ),
            graph_rule_of_kind(
                "//shop:client",
                "openapi_typescript_client",
                &["//shop:spec"],
                &[],
            ),
            graph_rule_of_kind(
                "//shop:client_shard",
                "openapi_codegen_shard",
                &["//shop:client"],
                &[],
            ),
        ]))]);
        let got = expand_codegen_roots("//shop:spec", &workspace, &query, &[]).expect("expand");
        assert_eq!(
            got,
            vec!["//shop:spec".to_owned(), "//shop:client_shard".to_owned()]
        );
    }

    #[test]
    fn a_graph_without_shards_keeps_the_single_label() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-empty-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[graph_rule_of_kind(
            "//generation:result_proto_rs",
            "rust_prost_library",
            &["//generation:result_proto"],
            &[],
        )]))]);
        let got = expand_codegen_roots("//generation:result_proto", &workspace, &query, &[])
            .expect("expand");
        assert_eq!(got, vec!["//generation:result_proto".to_owned()]);
        assert_no_rdeps(&query.calls());
    }

    #[test]
    fn expansion_failures_keep_the_bazel_status_and_excerpt() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-codegen-expand-fail-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::failed("\n  query failed: blah  \nmore\n")]);
        let err = expand_codegen_roots("//generation:result_proto", &workspace, &query, &[])
            .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "kind('rule', //...)".to_owned(),
                failure: QueryFailure::exit_for_test(2, "  query failed: blah\nmore"),
            }
        );
    }
}
