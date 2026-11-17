#[cfg(test)]
use std::path::Path;

use super::query::quote_label;

#[cfg(test)]
use super::QueryRunner;
use super::{ResolveError, RuleGraph, SelectionContext};

#[must_use]
pub fn set_expression(labels: &[String]) -> String {
    let quoted: Vec<String> = labels.iter().map(|label| quote_label(label)).collect();
    format!("set({})", quoted.join(" "))
}

/// # Errors
///
/// Returns an error when the operation fails.
#[cfg(test)]
pub fn map_owners_to_tests_with_selection(
    owners: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    selection: &SelectionContext,
) -> Result<Vec<String>, ResolveError> {
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let graph = RuleGraph::load(workspace, runner, selection.startup())?;
    tests_for_owners(owners, &graph, selection)
}

/// The tests that use the owners, read from the rule graph without another query.
///
/// The graph is unconfigured, so a configured selection keeps a test that reaches an owner
/// only through an inactive `select()` branch. That runs a test more than needed, never fewer,
/// and test runs skip incompatible targets.
pub(crate) fn tests_for_owners(
    owners: &[String],
    graph: &RuleGraph,
    selection: &SelectionContext,
) -> Result<Vec<String>, ResolveError> {
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let mut tests = graph.tests_of(&graph.test_candidates(owners));
    if selection.configured() {
        // A suite can name tests that never use the owners; a configured run only wants
        // the ones that do.
        let users = graph.users_of(owners);
        tests.retain(|test| users.contains(test.as_str()));
    }
    if tests.is_empty() {
        let mut sorted = owners.to_vec();
        sorted.sort();
        return Err(ResolveError::NoTests { owners: sorted });
    }
    Ok(tests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{FakeQuery, QueryResult};
    use crate::resolve::{NeverQuery, QueryFailure};
    use dx_testing::cli::{graph_rule, graph_rule_of_kind as kind, strings};

    fn lib_graph() -> QueryResult {
        FakeQuery::graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            kind("//pkg:unit", "py_test", &["//pkg:lib"], &[]),
            kind(
                "//pkg:e2e",
                "sh_test",
                &["//pkg:lib", "@@android+sdk//:core"],
                &[],
            ),
            graph_rule("//pkg:bin", &["//pkg:lib"], &[]),
            graph_rule("//other:bin", &["//other:main.py"], &[]),
        ])
    }

    const GRAPH: &str = "kind('rule', //...)";

    fn map(query: &FakeQuery, owners: &[&str], selection: &SelectionContext) -> Vec<String> {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-");
        map_owners_to_tests_with_selection(&strings(owners), scratch.path(), query, selection)
            .expect("map")
    }

    #[test]
    fn tests_come_from_the_graph_kinds_in_one_query() {
        for selection in [
            SelectionContext::unconfigured(&[]),
            SelectionContext::for_tests_configured(&strings(&["--platforms=//:x"]), &[]),
        ] {
            let query = FakeQuery::new(vec![lib_graph()]);
            let got = map(&query, &["//pkg:lib"], &selection);
            assert_eq!(got, strings(&["//pkg:e2e", "//pkg:unit"]));
            assert_eq!(query.expressions(), [GRAPH]);
        }
    }

    #[test]
    fn custom_test_rule_names_count_by_kind_not_target_name() {
        let query = FakeQuery::new(vec![FakeQuery::graph(&[
            graph_rule("//pkg:lib", &[], &[]),
            kind("//pkg:custom_check", "my_test", &["//pkg:lib"], &[]),
            graph_rule("//pkg:plain_test", &["//pkg:lib"], &[]),
        ])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//pkg:custom_check"]));
    }

    fn suite_graph(suite_tags: &[&str]) -> QueryResult {
        FakeQuery::graph(&[
            graph_rule("//pkg:lib", &[], &[]),
            kind("//pkg:unit", "py_test", &["//pkg:lib"], &[]),
            kind(
                "//pkg:flaky",
                "py_test",
                &["//pkg:lib"],
                &["manual", "flaky"],
            ),
            kind("//other:t", "py_test", &[], &[]),
            kind(
                "//pkg:suite",
                "test_suite",
                &["//pkg:unit", "//pkg:flaky", "//pkg:nested"],
                suite_tags,
            ),
            kind(
                "//pkg:nested",
                "test_suite",
                &["//other:t", "//pkg:suite"],
                &[],
            ),
        ])
    }

    #[test]
    fn suites_expand_to_their_tests_once_and_filter_by_tag() {
        let query = FakeQuery::new(vec![suite_graph(&[])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//other:t", "//pkg:flaky", "//pkg:unit"]));

        let query = FakeQuery::new(vec![suite_graph(&["-flaky"])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//other:t", "//pkg:unit"]));

        let query = FakeQuery::new(vec![suite_graph(&["medium", "manual"])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//other:t", "//pkg:flaky", "//pkg:unit"]));

        let query = FakeQuery::new(vec![suite_graph(&["small"])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(
            got,
            strings(&["//other:t", "//pkg:unit"]),
            "unit uses the owner and the untagged nested suite keeps other:t"
        );
    }

    #[test]
    fn configured_mapping_keeps_only_suite_tests_that_use_the_owner() {
        let query = FakeQuery::new(vec![suite_graph(&[])]);
        let got = map(
            &query,
            &["//pkg:lib"],
            &SelectionContext::for_tests_configured(&[], &[]),
        );
        assert_eq!(got, strings(&["//pkg:flaky", "//pkg:unit"]));
    }

    #[test]
    fn empty_test_mapping_suggests_explicit_label() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-empty-");
        let query = FakeQuery::new(vec![FakeQuery::graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            graph_rule("//pkg:bin", &["//pkg:lib"], &[]),
        ])]);
        let err = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            scratch.path(),
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect_err("empty mapping");
        assert_eq!(
            err,
            ResolveError::NoTests {
                owners: strings(&["//pkg:lib"]),
            }
        );
        assert!(
            err.to_string().contains("explicit test label"),
            "actionable: {err}"
        );
    }

    #[test]
    fn graph_failures_keep_the_bazel_status_and_excerpt() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-fail-");
        let query = FakeQuery::new(vec![FakeQuery::failed("\n  query failed: blah  \nmore\n")]);
        let err = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            scratch.path(),
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: GRAPH.to_owned(),
                failure: QueryFailure::exit_for_test(2, "  query failed: blah\nmore"),
            }
        );
    }

    #[test]
    fn empty_owners_map_to_no_tests_without_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-no-query-");
        let got = map_owners_to_tests_with_selection(
            &[],
            scratch.path(),
            &NeverQuery,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, [] as [String; 0]);
    }

    #[test]
    fn no_tests_error_renders_owners_and_guidance() {
        let error = ResolveError::NoTests {
            owners: strings(&["//a:lib", "//b:lib"]),
        };
        let text = error.to_string();
        assert!(text.contains("//a:lib //b:lib"), "{text}");
        assert!(text.contains("//pkg/..."), "{text}");
    }

    #[test]
    fn a_test_behind_a_manual_library_is_still_found() {
        let query = FakeQuery::new(vec![FakeQuery::graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            graph_rule("//pkg:wrapper", &["//pkg:lib"], &["manual"]),
            kind("//pkg:t", "py_test", &["//pkg:wrapper"], &[]),
        ])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//pkg:t"]));
    }

    #[test]
    fn a_manual_test_that_depends_on_the_owner_is_left_out() {
        let query = FakeQuery::new(vec![FakeQuery::graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            kind("//pkg:browser", "py_test", &["//pkg:lib"], &["manual"]),
            kind("//pkg:unit", "py_test", &["//pkg:lib"], &[]),
        ])]);
        let got = map(&query, &["//pkg:lib"], &SelectionContext::unconfigured(&[]));
        assert_eq!(got, strings(&["//pkg:unit"]));
    }

    #[test]
    fn a_manual_test_that_owns_the_file_is_found() {
        for selection in [
            SelectionContext::unconfigured(&[]),
            SelectionContext::for_tests_configured(&[], &[]),
        ] {
            let query = FakeQuery::new(vec![FakeQuery::graph(&[kind(
                "//pkg:manual_test",
                "py_test",
                &["//pkg:a.py"],
                &["manual"],
            )])]);
            let got = map(&query, &["//pkg:manual_test"], &selection);
            assert_eq!(got, strings(&["//pkg:manual_test"]));
            assert_eq!(query.expressions(), [GRAPH]);
        }
    }

    #[test]
    fn an_owner_missing_from_the_graph_reports_no_tests_after_the_graph_only() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-no-candidates-");
        for selection in [
            SelectionContext::unconfigured(&[]),
            SelectionContext::for_tests_configured(&[], &[]),
        ] {
            let query = FakeQuery::new(vec![FakeQuery::graph(&[graph_rule(
                "//other:bin",
                &["//other:main.py"],
                &[],
            )])]);
            let err = map_owners_to_tests_with_selection(
                &strings(&["//pkg:gone"]),
                scratch.path(),
                &query,
                &selection,
            )
            .expect_err("no candidates");
            assert_eq!(
                err,
                ResolveError::NoTests {
                    owners: strings(&["//pkg:gone"]),
                }
            );
            assert_eq!(query.expressions(), [GRAPH]);
        }
    }
}
