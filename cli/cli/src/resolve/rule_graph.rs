use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use super::{QueryRunner, ResolveError};

const EXPRESSION: &str = "kind('rule', //...)";

/// The workspace rule graph as Bazel reports it, without loading any external repo.
#[derive(Debug)]
pub(crate) struct RuleGraph {
    inputs: HashMap<String, Vec<String>>,
    users: HashMap<String, Vec<String>>,
    manual: HashSet<String>,
    kinds: HashMap<String, String>,
}

fn query_argv(startup_options: &[String]) -> Vec<String> {
    let mut argv = dx_process::startup_argv(startup_options);
    for flag in [
        "query",
        "--keep_going",
        "--output=streamed_jsonproto",
        "--proto:output_rule_attrs=tags",
        "--noproto:default_values",
        "--noproto:locations",
        "--noproto:instantiation_stack",
        "--",
        EXPRESSION,
    ] {
        argv.push(flag.to_owned());
    }
    argv
}

fn has_manual_tag(rule: &Value) -> bool {
    let Some(attributes) = rule.get("attribute").and_then(Value::as_array) else {
        return false;
    };
    attributes
        .iter()
        .filter(|attribute| attribute.get("name").and_then(Value::as_str) == Some("tags"))
        .filter_map(|attribute| attribute.get("stringListValue").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .any(|tag| {
            tag.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .any(|word| word == "manual")
        })
}

fn package_of(label: &str) -> &str {
    label.split_once(':').map_or(label, |(package, _)| package)
}

fn matches_scope(scope: &str, label: &str) -> bool {
    if let Some(root) = scope.strip_suffix("/...") {
        let package = package_of(label);
        return package == root || package.starts_with(&format!("{root}/"));
    }
    if let Some(package) = scope
        .strip_suffix(":all")
        .or_else(|| scope.strip_suffix(":*"))
    {
        return package_of(label) == package;
    }
    scope == label
}

impl RuleGraph {
    pub(crate) fn load(
        workspace: &Path,
        runner: &dyn QueryRunner,
        startup_options: &[String],
    ) -> Result<RuleGraph, ResolveError> {
        let argv = query_argv(startup_options);
        let result = ResolveError::run(runner, &argv, workspace, EXPRESSION)?;
        let text = String::from_utf8(result.stdout).map_err(|_| {
            ResolveError::query(
                EXPRESSION,
                super::QueryFailure::Output {
                    detail: "query output is not UTF-8".to_owned(),
                },
            )
        })?;
        let mut outputs: Vec<(String, Vec<String>)> = Vec::new();
        let mut graph = RuleGraph {
            inputs: HashMap::new(),
            users: HashMap::new(),
            manual: HashSet::new(),
            kinds: HashMap::new(),
        };
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let value: Value = serde_json::from_str(line).map_err(|error| {
                ResolveError::query(
                    EXPRESSION,
                    super::QueryFailure::Output {
                        detail: format!("query output is not JSON: {error}"),
                    },
                )
            })?;
            let Some(rule) = value.get("rule") else {
                continue;
            };
            let Some(name) = rule.get("name").and_then(Value::as_str) else {
                continue;
            };
            let inputs: Vec<String> = rule
                .get("ruleInput")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let produced: Vec<String> = rule
                .get("ruleOutput")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            if !produced.is_empty() {
                outputs.push((name.to_owned(), produced));
            }
            for input in &inputs {
                graph
                    .users
                    .entry(input.clone())
                    .or_default()
                    .push(name.to_owned());
            }
            if has_manual_tag(rule) {
                graph.manual.insert(name.to_owned());
            }
            if let Some(kind) = rule.get("ruleClass").and_then(Value::as_str) {
                graph.kinds.insert(name.to_owned(), kind.to_owned());
            }
            graph.inputs.insert(name.to_owned(), inputs);
        }
        for (name, produced) in outputs {
            let consumers: Vec<String> = produced
                .iter()
                .filter_map(|output| graph.users.get(output))
                .flatten()
                .cloned()
                .collect();
            if !consumers.is_empty() {
                graph.users.entry(name).or_default().extend(consumers);
            }
        }
        Ok(graph)
    }

    pub(crate) fn owners_of(&self, labels: &[String]) -> Vec<String> {
        let mut owners: Vec<String> = labels
            .iter()
            .filter_map(|label| self.users.get(label))
            .flatten()
            .cloned()
            .collect();
        owners.sort();
        owners.dedup();
        owners
    }

    pub(crate) fn inputs_of(&self, rules: &[String]) -> HashSet<&str> {
        rules
            .iter()
            .filter_map(|rule| self.inputs.get(rule))
            .flatten()
            .map(String::as_str)
            .collect()
    }

    /// External labels that rules in the graph use.
    pub(crate) fn external_inputs(&self) -> impl Iterator<Item = &str> {
        self.users
            .keys()
            .map(String::as_str)
            .filter(|label| label.starts_with("@@"))
    }

    /// Rules affected by the scopes, without manual rules.
    pub(crate) fn affected(&self, scopes: &[String]) -> Vec<String> {
        self.keep(self.reach(scopes), |label| !self.manual.contains(label))
    }

    /// Rules that may hold tests for the owners, keeping manual owners.
    pub(crate) fn test_candidates(&self, owners: &[String]) -> Vec<String> {
        self.keep(self.reach(owners), |label| {
            !self.manual.contains(label) || owners.iter().any(|owner| owner == label)
        })
    }

    /// Rules of the kind that use the label, keeping manual rules.
    pub(crate) fn dependents_of_kind(
        &self,
        label: &str,
        kind: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        self.keep(self.reach(&self.seeds_for(label)), |rule| {
            self.kinds.get(rule).is_some_and(|class| kind(class))
        })
    }

    /// Rules of the kind reached from the seed labels, keeping manual rules.
    pub(crate) fn reached_of_kind(
        &self,
        seeds: &[String],
        kind: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        self.keep(self.reach(seeds), |rule| {
            self.kinds.get(rule).is_some_and(|class| kind(class))
        })
    }

    /// Graph labels that stand for the label, with apparent repos resolved.
    fn seeds_for(&self, label: &str) -> Vec<String> {
        if let Some(rest) = label
            .strip_prefix("@@//")
            .or_else(|| label.strip_prefix("@//"))
        {
            return vec![format!("//{rest}")];
        }
        if label.starts_with("//") || label.starts_with("@@") {
            return vec![label.to_owned()];
        }
        let Some((name, rest)) = label
            .strip_prefix('@')
            .and_then(|apparent| apparent.split_once("//"))
        else {
            return vec![label.to_owned()];
        };
        let mut seeds: Vec<String> = self
            .external_inputs()
            .filter(|input| {
                input
                    .strip_prefix("@@")
                    .and_then(|canonical| canonical.split_once("//"))
                    .is_some_and(|(repo, path)| {
                        path == rest
                            && (repo == name
                                || repo.strip_suffix('+') == Some(name)
                                || repo.ends_with(&format!("+{name}"))
                                || repo.ends_with(&format!("~{name}")))
                    })
            })
            .map(str::to_owned)
            .collect();
        if seeds.is_empty() {
            seeds.push(label.to_owned());
        }
        seeds.sort();
        seeds
    }

    /// Rules in the graph that pass the filter, sorted.
    fn keep(&self, reached: HashSet<&str>, wanted: impl Fn(&str) -> bool) -> Vec<String> {
        let mut rules: Vec<String> = reached
            .into_iter()
            .filter(|label| self.inputs.contains_key(*label) && wanted(label))
            .map(str::to_owned)
            .collect();
        rules.sort();
        rules
    }

    /// Labels the scopes seed plus every rule that uses them.
    fn reach<'a>(&'a self, scopes: &'a [String]) -> HashSet<&'a str> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut pending: Vec<&str> = Vec::new();
        for scope in scopes {
            if self.users.contains_key(scope.as_str()) {
                pending.push(scope);
            }
            for rule in self.inputs.keys() {
                if matches_scope(scope, rule) {
                    pending.push(rule);
                }
            }
        }
        while let Some(label) = pending.pop() {
            if !seen.insert(label) {
                continue;
            }
            if let Some(users) = self.users.get(label) {
                pending.extend(users.iter().map(String::as_str));
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::QueryResult;
    use dx_testing::cli::{graph_rule as rule, graph_rule_of_kind, graph_stdout, strings};
    use std::cell::RefCell;

    pub(crate) struct Answer {
        pub(crate) code: Option<i32>,
        pub(crate) stdout: String,
        pub(crate) argv: RefCell<Vec<Vec<String>>>,
    }

    impl QueryRunner for Answer {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> std::io::Result<QueryResult> {
            self.argv.borrow_mut().push(argv.to_vec());
            Ok(QueryResult {
                code: self.code,
                stdout: self.stdout.clone().into_bytes(),
                stderr: b"ERROR: no such package\n".to_vec(),
            })
        }
    }

    fn graph(lines: &[String]) -> RuleGraph {
        let answer = Answer {
            code: Some(0),
            stdout: graph_stdout(lines),
            argv: RefCell::new(Vec::new()),
        };
        RuleGraph::load(Path::new("/w"), &answer, &[]).expect("graph")
    }

    fn sample() -> RuleGraph {
        graph(&[
            rule("//a:lib", &["//a:a.txt", "//a:BUILD.bazel"], &[]),
            rule("//a:t", &["//a:lib"], &["premanual", "size"]),
            rule("//b:t", &["//a:lib", "@@ruby+x//:ruby"], &[]),
            rule("//c:m", &["//a:lib"], &["manual"]),
            rule("//c:t", &["//c:m"], &[]),
            rule("//d:lone", &["@@android+sdk//:core"], &[]),
        ])
    }

    fn shard_graph() -> RuleGraph {
        graph(&[
            graph_rule_of_kind(
                "//gen:schema",
                "proto_library",
                &["//gen:schema.proto"],
                &[],
            ),
            graph_rule_of_kind(
                "//gen:schema_rs",
                "rust_prost_library",
                &["//gen:schema"],
                &[],
            ),
            graph_rule_of_kind(
                "//gen:shard",
                "prost_codegen_shard",
                &["//gen:schema_rs"],
                &[],
            ),
            graph_rule_of_kind(
                "//gen:held",
                "dx_codegen_shard",
                &["//gen:schema"],
                &["manual"],
            ),
            graph_rule_of_kind("//gen:bin", "rust_binary", &["//gen:schema_rs"], &[]),
            rule("//gen:untyped_shard", &["//gen:schema"], &[]),
            graph_rule_of_kind(
                "//ext:shard",
                "prost_codegen_shard",
                &["@@repo+//pkg:schema"],
                &[],
            ),
        ])
    }

    #[test]
    fn reached_of_kind_walks_from_seeds_to_rules_of_the_kind() {
        let graph = shard_graph();
        let seeds = strings(&["//gen:schema"]);
        assert_eq!(
            graph.reached_of_kind(&seeds, |kind| kind == "rust_binary"),
            ["//gen:bin"]
        );
        assert!(graph
            .reached_of_kind(&seeds, |kind| kind == "toolchain")
            .is_empty());
        assert!(graph
            .reached_of_kind(&[], |kind| kind == "rust_binary")
            .is_empty());
    }

    #[test]
    fn shards_are_found_through_a_transitive_chain() {
        let graph = shard_graph();
        let shards = graph.dependents_of_kind("//gen:schema", dx_codegen::is_shard_kind);
        assert!(shards.contains(&"//gen:shard".to_owned()), "{shards:?}");
        assert_eq!(
            graph.dependents_of_kind("//gen:schema_rs", dx_codegen::is_shard_kind),
            ["//gen:shard"]
        );
    }

    #[test]
    fn manual_shards_are_kept() {
        let graph = shard_graph();
        assert_eq!(
            graph.dependents_of_kind("//gen:schema", dx_codegen::is_shard_kind),
            ["//gen:held", "//gen:shard"]
        );
    }

    #[test]
    fn non_shard_dependents_are_dropped() {
        let graph = shard_graph();
        let shards = graph.dependents_of_kind("//gen:schema", dx_codegen::is_shard_kind);
        for dropped in ["//gen:schema", "//gen:schema_rs", "//gen:bin"] {
            assert!(
                !shards.contains(&dropped.to_owned()),
                "{dropped}: {shards:?}"
            );
        }
        assert_eq!(
            graph.dependents_of_kind("//gen:schema", |kind| kind == "rust_binary"),
            ["//gen:bin"]
        );
    }

    #[test]
    fn apparent_repo_seeds_reach_canonical_inputs() {
        let graph = shard_graph();
        for label in ["@repo//pkg:schema", "@@repo+//pkg:schema"] {
            assert_eq!(
                graph.dependents_of_kind(label, dx_codegen::is_shard_kind),
                ["//ext:shard"],
                "{label}"
            );
        }
        for label in ["@//gen:schema_rs", "@@//gen:schema_rs"] {
            assert_eq!(
                graph.dependents_of_kind(label, dx_codegen::is_shard_kind),
                ["//gen:shard"],
                "{label}"
            );
        }
        assert!(graph
            .dependents_of_kind("@other//pkg:schema", dx_codegen::is_shard_kind)
            .is_empty());
    }

    #[test]
    fn unknown_labels_have_no_shards() {
        let graph = shard_graph();
        assert!(graph
            .dependents_of_kind("//gone:schema", dx_codegen::is_shard_kind)
            .is_empty());
    }

    #[test]
    fn rules_without_a_rule_class_are_never_shards() {
        let graph = shard_graph();
        let shards = graph.dependents_of_kind("//gen:schema", |_| true);
        assert!(
            !shards.contains(&"//gen:untyped_shard".to_owned()),
            "{shards:?}"
        );
        assert!(shards.contains(&"//gen:schema_rs".to_owned()), "{shards:?}");
    }

    #[test]
    fn query_stays_inside_the_workspace_and_keeps_implicit_and_tool_deps() {
        let answer = Answer {
            code: Some(0),
            stdout: String::new(),
            argv: RefCell::new(Vec::new()),
        };
        RuleGraph::load(Path::new("/w"), &answer, &strings(&["--output_base=/o"])).expect("graph");
        let argv = answer.argv.borrow()[0].clone();
        assert!(argv.contains(&"--keep_going".to_owned()), "{argv:?}");
        for flag in ["--noimplicit_deps", "--notool_deps"] {
            assert!(!argv.contains(&flag.to_owned()), "{flag}: {argv:?}");
        }
        assert!(argv.contains(&"--output_base=/o".to_owned()));
        assert_eq!(argv.last().map(String::as_str), Some("kind('rule', //...)"));
        assert!(!argv.iter().any(|arg| arg.contains("rdeps")), "{argv:?}");
    }

    #[test]
    fn external_repos_in_the_inputs_are_never_followed() {
        let graph = sample();
        assert_eq!(
            graph.affected(&strings(&["//a:a.txt"])),
            ["//a:lib", "//a:t", "//b:t", "//c:t"]
        );
        assert!(graph
            .affected(&strings(&["//d:lone"]))
            .contains(&"//d:lone".to_owned()));
    }

    #[test]
    fn external_input_seeds_select_their_users_and_rdeps() {
        let full = sample();
        let mut external: Vec<&str> = full.external_inputs().collect();
        external.sort_unstable();
        assert_eq!(external, ["@@android+sdk//:core", "@@ruby+x//:ruby"]);
        assert_eq!(full.affected(&strings(&["@@ruby+x//:ruby"])), ["//b:t"]);
        let small = graph(&[
            rule("//a:lib", &["@@x+//:y"], &[]),
            rule("//a:t", &["//a:lib"], &[]),
            rule("//b:t", &["//b:b.txt"], &[]),
        ]);
        assert_eq!(
            small.affected(&strings(&["@@x+//:y"])),
            ["//a:lib", "//a:t"]
        );
    }

    #[test]
    fn manual_rules_are_dropped_but_their_dependents_are_not() {
        let graph = sample();
        let affected = graph.affected(&strings(&["//a:lib"]));
        assert!(!affected.contains(&"//c:m".to_owned()), "{affected:?}");
        assert!(affected.contains(&"//c:t".to_owned()), "{affected:?}");
        assert!(affected.contains(&"//a:t".to_owned()), "{affected:?}");
    }

    #[test]
    fn owners_stay_test_candidates_even_when_manual() {
        let graph = sample();
        assert_eq!(
            graph.test_candidates(&strings(&["//c:m"])),
            ["//c:m", "//c:t"]
        );
        let candidates = graph.test_candidates(&strings(&["//a:lib"]));
        assert!(candidates.contains(&"//a:t".to_owned()), "{candidates:?}");
        assert!(candidates.contains(&"//c:t".to_owned()), "{candidates:?}");
        assert!(!candidates.contains(&"//c:m".to_owned()), "{candidates:?}");
        assert!(graph.test_candidates(&strings(&["//x:gone"])).is_empty());
    }

    #[test]
    fn package_scopes_seed_every_rule_they_hold() {
        let graph = sample();
        assert_eq!(graph.affected(&strings(&["//d/..."])), ["//d:lone"]);
        assert!(graph.affected(&strings(&["//e/..."])).is_empty());
        assert_eq!(graph.affected(&strings(&["//d:all"])), ["//d:lone"]);
        assert_eq!(graph.affected(&strings(&["//c/..."])), ["//c:t"]);
        assert_eq!(graph.affected(&strings(&["//b:*"])), ["//b:t"]);
    }

    #[test]
    fn owners_are_the_direct_users_of_a_file() {
        let graph = sample();
        assert_eq!(graph.owners_of(&strings(&["//a:a.txt"])), ["//a:lib"]);
        assert!(graph.owners_of(&strings(&["//a:nobody.txt"])).is_empty());
        let owners = strings(&["//a:lib"]);
        assert!(graph.inputs_of(&owners).contains("//a:a.txt"));
    }

    #[test]
    fn a_partial_or_failed_result_is_an_error() {
        let line = rule("//a:lib", &[], &[]);
        for code in [3, 7] {
            for stdout in [String::new(), line.clone()] {
                let failed = Answer {
                    code: Some(code),
                    stdout,
                    argv: RefCell::new(Vec::new()),
                };
                assert!(matches!(
                    RuleGraph::load(Path::new("/w"), &failed, &[]),
                    Err(ResolveError::QueryFailed { .. })
                ));
            }
        }
    }

    #[test]
    fn tool_and_implicit_inputs_select_their_users() {
        let graph = graph(&[
            rule("//tools:gen", &["//tools:gen.py"], &[]),
            rule("//app:g", &["//tools:gen"], &[]),
            rule(
                "//app:r",
                &["//app:r.txt", "@@bazel_tools//tools/cpp:toolchain"],
                &[],
            ),
        ]);
        assert_eq!(
            graph.affected(&strings(&["//tools:gen.py"])),
            ["//app:g", "//tools:gen"]
        );
    }

    #[test]
    fn output_files_link_their_generating_rule_to_consumers() {
        let produced = serde_json::json!({
            "type": "RULE",
            "rule": {
                "name": "//app:g",
                "ruleInput": ["//app:in.txt"],
                "ruleOutput": ["//app:g.txt"],
            }
        })
        .to_string();
        let graph = graph(&[
            produced,
            rule("//app:fg", &["//app:g.txt"], &[]),
            rule("//app:t", &["//app:fg"], &[]),
        ]);
        assert_eq!(
            graph.affected(&strings(&["//app:in.txt"])),
            ["//app:fg", "//app:g", "//app:t"]
        );
    }

    #[test]
    fn unreadable_output_is_a_typed_failure() {
        for bytes in [vec![0xff, 0xfe], b"not json\n".to_vec()] {
            struct Raw(Vec<u8>);
            impl QueryRunner for Raw {
                fn run_query(&self, _: &[String], _: &Path) -> std::io::Result<QueryResult> {
                    Ok(QueryResult {
                        code: Some(0),
                        stdout: self.0.clone(),
                        stderr: Vec::new(),
                    })
                }
            }
            assert!(matches!(
                RuleGraph::load(Path::new("/w"), &Raw(bytes), &[]),
                Err(ResolveError::QueryFailed { .. })
            ));
        }
    }

    #[test]
    fn a_missing_bazel_is_a_launch_failure() {
        struct Missing;
        impl QueryRunner for Missing {
            fn run_query(&self, _: &[String], _: &Path) -> std::io::Result<QueryResult> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no bazel",
                ))
            }
        }
        assert!(matches!(
            RuleGraph::load(Path::new("/w"), &Missing, &[]),
            Err(ResolveError::QueryFailed { .. })
        ));
    }
}
