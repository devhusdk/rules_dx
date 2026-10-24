use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use super::{QueryRunner, ResolveError};

const EXPRESSION: &str = "kind('rule', //...)";

/// The workspace rule graph as Bazel reports it, without loading any external repo.
pub(crate) struct RuleGraph {
    inputs: HashMap<String, Vec<String>>,
    users: HashMap<String, Vec<String>>,
    manual: HashSet<String>,
}

fn query_argv(startup_options: &[String]) -> Vec<String> {
    let mut argv = dx_process::startup_argv(startup_options);
    for flag in [
        "query",
        "--keep_going",
        "--noimplicit_deps",
        "--notool_deps",
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
        let result = runner.run_query(&argv, workspace).map_err(|error| {
            ResolveError::query(EXPRESSION, super::QueryFailure::launch(&error))
        })?;
        let partial = result.code == Some(3) && !result.stdout.is_empty();
        if result.code != Some(0) && !partial {
            return Err(ResolveError::query(
                EXPRESSION,
                super::QueryFailure::exited(&result),
            ));
        }
        let text = String::from_utf8(result.stdout).map_err(|_| {
            ResolveError::query(
                EXPRESSION,
                super::QueryFailure::Output {
                    detail: "query output is not UTF-8".to_owned(),
                },
            )
        })?;
        let mut graph = RuleGraph {
            inputs: HashMap::new(),
            users: HashMap::new(),
            manual: HashSet::new(),
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
            graph.inputs.insert(name.to_owned(), inputs);
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

    pub(crate) fn affected(&self, scopes: &[String]) -> Vec<String> {
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
        let mut rules: Vec<String> = seen
            .into_iter()
            .filter(|label| self.inputs.contains_key(*label) && !self.manual.contains(*label))
            .map(str::to_owned)
            .collect();
        rules.sort();
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::QueryResult;
    use crate::test_support::strings;
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

    fn rule(name: &str, inputs: &[&str], tags: &[&str]) -> String {
        serde_json::json!({
            "type": "RULE",
            "rule": {
                "name": name,
                "ruleInput": inputs,
                "attribute": [{"name": "tags", "stringListValue": tags}],
            }
        })
        .to_string()
    }

    fn graph(lines: &[String]) -> RuleGraph {
        let answer = Answer {
            code: Some(0),
            stdout: lines.join("\n"),
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

    #[test]
    fn query_stays_inside_the_workspace_and_keeps_going() {
        let answer = Answer {
            code: Some(0),
            stdout: String::new(),
            argv: RefCell::new(Vec::new()),
        };
        RuleGraph::load(Path::new("/w"), &answer, &strings(&["--output_base=/o"])).expect("graph");
        let argv = answer.argv.borrow()[0].clone();
        for flag in ["--keep_going", "--noimplicit_deps", "--notool_deps"] {
            assert!(argv.contains(&flag.to_owned()), "{flag}: {argv:?}");
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
    fn manual_rules_are_dropped_but_their_dependents_are_not() {
        let graph = sample();
        let affected = graph.affected(&strings(&["//a:lib"]));
        assert!(!affected.contains(&"//c:m".to_owned()), "{affected:?}");
        assert!(affected.contains(&"//c:t".to_owned()), "{affected:?}");
        assert!(affected.contains(&"//a:t".to_owned()), "{affected:?}");
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
    fn a_partial_result_is_used_and_a_hard_failure_is_not() {
        let line = rule("//a:lib", &[], &[]);
        let partial = Answer {
            code: Some(3),
            stdout: line.clone(),
            argv: RefCell::new(Vec::new()),
        };
        assert!(RuleGraph::load(Path::new("/w"), &partial, &[]).is_ok());
        for stdout in [String::new(), line] {
            let failed = Answer {
                code: Some(7),
                stdout,
                argv: RefCell::new(Vec::new()),
            };
            assert!(matches!(
                RuleGraph::load(Path::new("/w"), &failed, &[]),
                Err(ResolveError::QueryFailed { .. })
            ));
        }
        let empty_partial = Answer {
            code: Some(3),
            stdout: String::new(),
            argv: RefCell::new(Vec::new()),
        };
        assert!(RuleGraph::load(Path::new("/w"), &empty_partial, &[]).is_err());
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
