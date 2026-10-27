use std::path::Path;

use super::query::quote_label;

use super::{
    parse_owners, quote_set, QueryFailure, QueryRunner, ResolveError, RuleGraph, SelectionContext,
};

pub(crate) fn strip_configuration(line: &str) -> &str {
    let line = line.trim();
    match line.rfind(" (") {
        Some(index) if line.ends_with(')') => &line[..index],
        _ => line,
    }
}

pub fn set_expression(labels: &[String]) -> String {
    let quoted: Vec<String> = labels.iter().map(|label| quote_label(label)).collect();
    format!("set({})", quoted.join(" "))
}

fn prepare_failure(expression: &str, detail: String) -> ResolveError {
    ResolveError::query(expression, QueryFailure::Prepare { detail })
}

fn run_labels(
    runner: &dyn QueryRunner,
    selection: &SelectionContext,
    verb: &str,
    expression: &str,
    workspace: &Path,
) -> Result<Vec<String>, ResolveError> {
    let mut file = tempfile::NamedTempFile::new().map_err(|error| {
        prepare_failure(expression, format!("cannot write the query file: {error}"))
    })?;
    std::io::Write::write_all(&mut file, expression.as_bytes()).map_err(|error| {
        prepare_failure(expression, format!("cannot write the query file: {error}"))
    })?;
    let argv = selection
        .selection_query_file_argv(verb, file.path())
        .map_err(|error| prepare_failure(expression, error.to_string()))?;
    let result = ResolveError::run(runner, &argv, workspace, expression)?;
    let lines = parse_owners(&result.stdout, expression)?;
    let mut labels: Vec<String> = lines
        .iter()
        .map(|line| strip_configuration(line).to_owned())
        .collect();
    labels.sort();
    labels.dedup();
    Ok(labels)
}

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
    tests_for_owners(owners, &graph, workspace, runner, selection)
}

pub(crate) fn tests_for_owners(
    owners: &[String],
    graph: &RuleGraph,
    workspace: &Path,
    runner: &dyn QueryRunner,
    selection: &SelectionContext,
) -> Result<Vec<String>, ResolveError> {
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let no_tests = || {
        let mut sorted = owners.to_vec();
        sorted.sort();
        ResolveError::NoTests { owners: sorted }
    };
    let candidates = graph.test_candidates(owners);
    if candidates.is_empty() {
        return Err(no_tests());
    }
    let expansion = format!("tests({})", set_expression(&candidates));
    let expanded = run_labels(
        runner,
        &selection.unconfigured_view(),
        "query",
        &expansion,
        workspace,
    )?;
    if expanded.is_empty() {
        return Err(no_tests());
    }
    if !selection.configured() {
        return Ok(expanded);
    }
    let reach = format!(
        "rdeps({}, set({}))",
        set_expression(&expanded),
        quote_set(owners)
    );
    let reached = run_labels(runner, selection, "cquery", &reach, workspace)?;
    let tests: Vec<String> = reached
        .into_iter()
        .filter(|label| expanded.binary_search(label).is_ok())
        .collect();
    if tests.is_empty() {
        return Err(no_tests());
    }
    Ok(tests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::NeverQuery;
    use crate::resolve::{FakeQuery, QueryResult};
    use dx_testing::cli::{graph_rule, strings};

    fn lib_graph() -> QueryResult {
        FakeQuery::graph(&[
            graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
            graph_rule("//pkg:unit", &["//pkg:lib"], &[]),
            graph_rule("//pkg:e2e", &["//pkg:lib", "@@android+sdk//:core"], &[]),
            graph_rule("//other:bin", &["//other:main.py"], &[]),
        ])
    }

    const GRAPH: &str = "kind('rule', //...)";

    const LIB_TESTS: &str = "tests(set(\"//pkg:e2e\" \"//pkg:lib\" \"//pkg:unit\"))";

    #[test]
    fn test_mapping_queries_tests_of_graph_dependents() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::ok("//pkg:unit\n//pkg:e2e\n//pkg:unit\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:e2e", "//pkg:unit"]));
        assert_eq!(query.expressions(), [GRAPH, LIB_TESTS]);
        let calls = query.calls();
        assert!(calls.iter().all(|(_, cwd)| *cwd == workspace));
        assert_eq!(
            calls[1].0[..4],
            strings(&["bazel", "--nohome_rc", "--nosystem_rc", "query"])
        );
        assert_eq!(calls[1].0.len(), 5, "{:?}", calls[1].0);
        assert!(calls[1].0[4].starts_with("--query_file="));
    }

    #[test]
    fn test_mapping_sorts_candidates_in_set_expression() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-order-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::graph(&[
                graph_rule("//z:lib", &[], &[]),
                graph_rule("//a:lib", &[], &[]),
            ]),
            FakeQuery::ok("//t:t\n"),
        ]);
        map_owners_to_tests_with_selection(
            &strings(&["//z:lib", "//a:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(
            query.expressions()[1],
            "tests(set(\"//a:lib\" \"//z:lib\"))"
        );
    }

    #[test]
    fn empty_test_mapping_suggests_explicit_label() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-empty-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![lib_graph(), FakeQuery::ok("\n")]);
        let err = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
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
    fn test_mapping_failures_keep_the_bazel_status_and_excerpt() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-fail-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::failed("\n  query failed: blah  \nmore\n"),
        ]);
        let err = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: LIB_TESTS.to_owned(),
                failure: QueryFailure::exit_for_test(2, "  query failed: blah\nmore"),
            }
        );
    }

    #[test]
    fn test_mapping_carries_startup_options_before_the_verb() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-startup-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![lib_graph(), FakeQuery::ok("//pkg:unit\n")]);
        let startup = vec!["--output_base=/tmp/a".to_owned()];
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&startup),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:unit"]));
        let calls = query.calls();
        assert_eq!(calls.len(), 2);
        for (argv, _) in &calls {
            assert_eq!(
                argv[..5],
                strings(&[
                    "bazel",
                    "--nohome_rc",
                    "--nosystem_rc",
                    "--output_base=/tmp/a",
                    "query",
                ])
            );
        }
    }

    #[test]
    fn empty_owners_map_to_no_tests_without_query() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-no-query-");
        let workspace = scratch.path().to_path_buf();
        let query = NeverQuery;
        let got = map_owners_to_tests_with_selection(
            &[],
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert!(got.is_empty());
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
    fn test_mapping_keeps_custom_target_names_without_suffix_assumption() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-custom-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::graph(&[
                graph_rule("//pkg:lib", &[], &[]),
                graph_rule("//pkg:custom_check", &["//pkg:lib"], &[]),
                graph_rule("//pkg:plain_test", &["//pkg:lib"], &[]),
            ]),
            FakeQuery::ok("//pkg:custom_check\n//pkg:plain_test\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:custom_check", "//pkg:plain_test"]));
        assert_eq!(
            query.expressions()[1],
            "tests(set(\"//pkg:custom_check\" \"//pkg:lib\" \"//pkg:plain_test\"))"
        );
    }

    #[test]
    fn test_mapping_dedupes_suite_expansion() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-suite-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::ok("//pkg:custom_check\n//pkg:custom_check\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:custom_check"]));
    }

    #[test]
    fn a_test_behind_a_manual_library_is_still_found() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-manual-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::graph(&[
                graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
                graph_rule("//pkg:wrapper", &["//pkg:lib"], &["manual"]),
                graph_rule("//pkg:t", &["//pkg:wrapper"], &[]),
            ]),
            FakeQuery::ok("//pkg:t\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:t"]));
        assert_eq!(
            query.expressions()[1],
            "tests(set(\"//pkg:lib\" \"//pkg:t\"))"
        );
    }

    #[test]
    fn a_manual_test_that_depends_on_the_owner_is_left_out() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-manual-dependent-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::graph(&[
                graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
                graph_rule("//pkg:browser", &["//pkg:lib"], &["manual"]),
                graph_rule("//pkg:unit", &["//pkg:lib"], &[]),
            ]),
            FakeQuery::ok("//pkg:unit\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:unit"]));
        assert_eq!(
            query.expressions()[1],
            "tests(set(\"//pkg:lib\" \"//pkg:unit\"))"
        );
    }

    fn manual_test_graph() -> QueryResult {
        FakeQuery::graph(&[graph_rule(
            "//pkg:manual_test",
            &["//pkg:a.py"],
            &["manual"],
        )])
    }

    #[test]
    fn a_manual_test_that_owns_the_file_is_found() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-manual-owner-");
        let workspace = scratch.path().to_path_buf();
        let owners = strings(&["//pkg:manual_test"]);
        let query = FakeQuery::new(vec![
            manual_test_graph(),
            FakeQuery::ok("//pkg:manual_test\n"),
        ]);
        let got = map_owners_to_tests_with_selection(
            &owners,
            &workspace,
            &query,
            &SelectionContext::unconfigured(&[]),
        )
        .expect("map");
        assert_eq!(got, owners);
        assert_eq!(
            query.expressions(),
            [GRAPH, "tests(set(\"//pkg:manual_test\"))"]
        );

        let query = FakeQuery::new(vec![
            manual_test_graph(),
            FakeQuery::ok("//pkg:manual_test\n"),
            FakeQuery::ok("//pkg:manual_test (abc)\n"),
        ]);
        let selection = SelectionContext::for_tests_configured(&[], &[]);
        let got = map_owners_to_tests_with_selection(&owners, &workspace, &query, &selection)
            .expect("map");
        assert_eq!(got, owners);
        assert_eq!(query.verbs(), ["query", "query", "cquery"]);
        assert_eq!(
            query.expressions()[2],
            "rdeps(set(\"//pkg:manual_test\"), set(\"//pkg:manual_test\"))"
        );
    }

    #[test]
    fn an_owner_missing_from_the_graph_reports_no_tests_after_the_graph_only() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-no-candidates-");
        let workspace = scratch.path().to_path_buf();
        let unrelated =
            || FakeQuery::graph(&[graph_rule("//other:bin", &["//other:main.py"], &[])]);
        for selection in [
            SelectionContext::unconfigured(&[]),
            SelectionContext::for_tests_configured(&[], &[]),
        ] {
            let query = FakeQuery::new(vec![unrelated()]);
            let err = map_owners_to_tests_with_selection(
                &strings(&["//pkg:gone"]),
                &workspace,
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

    #[test]
    fn configured_mapping_never_sends_tests_to_cquery() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-configured-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::ok("//pkg:e2e\n//pkg:unit\n"),
            FakeQuery::ok("//pkg:lib (abc123)\n//pkg:unit (abc123)\n//pkg:e2e (abc123)\n"),
        ]);
        let selection =
            SelectionContext::for_tests_configured(&strings(&["--platforms=//:x"]), &[]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &selection,
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:e2e", "//pkg:unit"]));
        assert_eq!(query.verbs(), ["query", "query", "cquery"]);
        assert_eq!(
            query.expressions(),
            [
                GRAPH,
                LIB_TESTS,
                "rdeps(set(\"//pkg:e2e\" \"//pkg:unit\"), set(\"//pkg:lib\"))",
            ]
        );
        assert!(query.calls()[2].0.contains(&"--platforms=//:x".to_owned()));
        assert!(!query.expressions()[2].contains("tests("));
    }

    #[test]
    fn configured_mapping_drops_tests_behind_an_inactive_select() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-select-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::ok("//pkg:e2e\n//pkg:unit\n"),
            FakeQuery::ok("//pkg:lib (abc123)\n//pkg:unit (abc123)\n"),
        ]);
        let selection = SelectionContext::for_tests_configured(&[], &[]);
        let got = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &selection,
        )
        .expect("map");
        assert_eq!(got, strings(&["//pkg:unit"]));
    }

    #[test]
    fn configured_mapping_without_reached_tests_reports_no_tests() {
        let scratch = dx_test_scratch::scratch("dx-resolve-test-test-map-configured-none-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            lib_graph(),
            FakeQuery::ok("//pkg:unit\n"),
            FakeQuery::ok("//pkg:lib (abc123)\n"),
        ]);
        let selection = SelectionContext::for_tests_configured(&[], &[]);
        let err = map_owners_to_tests_with_selection(
            &strings(&["//pkg:lib"]),
            &workspace,
            &query,
            &selection,
        )
        .expect_err("none");
        assert_eq!(
            err,
            ResolveError::NoTests {
                owners: strings(&["//pkg:lib"]),
            }
        );
    }

    #[test]
    fn strip_configuration_drops_hash_suffix_only() {
        assert_eq!(strip_configuration("//a:b (abc)"), "//a:b");
        assert_eq!(strip_configuration("//a:b"), "//a:b");
    }
}
