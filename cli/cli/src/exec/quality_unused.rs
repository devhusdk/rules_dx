use super::results::Collected;
use dx_output::DiagnosticEvent;
use dx_plan::resolve::{quote_set, run_label_query_file, QueryRunner, ResolveError};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The requested targets that no other requested target reaches through `deps`.
pub(crate) fn top_level_targets(
    targets: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let expression = format!(
        "let p = {} in $p except deps(labels(deps, $p))",
        quote_set(targets)
    );
    run_label_query_file(&expression, "unused", workspace, runner, startup_options)
}

type Key = (Option<String>, Option<(u64, u64)>, String, String);

fn key(event: &DiagnosticEvent) -> Key {
    (
        event.path.clone(),
        event.range,
        event.tool.clone(),
        event.message.clone(),
    )
}

/// Keeps a finding only when every analyzed target whose closure holds its file reports it.
pub(crate) fn keep_agreed_findings(collected: &mut Collected) {
    let mut reporters: BTreeMap<Key, BTreeSet<&str>> = BTreeMap::new();
    for event in &collected.initial {
        let targets = reporters.entry(key(event)).or_default();
        targets.extend(event.origins.iter().map(|origin| origin.target.as_str()));
    }
    let agreed: BTreeSet<Key> = reporters
        .iter()
        .filter(|(key, found)| {
            let Some(path) = &key.0 else {
                return true;
            };
            collected
                .closures
                .iter()
                .filter(|(_, files)| files.contains(path))
                .all(|(target, _)| found.contains(target.as_str()))
        })
        .map(|(key, _)| key.clone())
        .collect();
    let mut seen = BTreeSet::new();
    let mut filter = |events: &mut Vec<DiagnosticEvent>| {
        seen.clear();
        events.retain(|event| {
            let key = key(event);
            agreed.contains(&key) && seen.insert(key)
        });
    };
    filter(&mut collected.initial);
    filter(&mut collected.terminal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_output::{Origin, Severity, Snapshot};

    fn finding(path: &str, name: &str, target: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            path: Some(path.to_owned()),
            range: Some((0, 1)),
            snapshot: Snapshot::Initial,
            origins: vec![Origin {
                producer: "p".to_owned(),
                target: target.to_owned(),
                configuration: None,
            }],
            ..DiagnosticEvent::new(Severity::Warning, "vulture", name)
        }
    }

    fn collected(closures: &[(&str, &[&str])], initial: Vec<DiagnosticEvent>) -> Collected {
        Collected {
            tools: Vec::new(),
            ran: BTreeSet::new(),
            analyzed: BTreeSet::new(),
            closures: closures
                .iter()
                .map(|(target, files)| {
                    (
                        (*target).to_owned(),
                        files.iter().map(|f| (*f).to_owned()).collect(),
                    )
                })
                .collect(),
            terminal: initial.clone(),
            initial,
            changes: Vec::new(),
            terminal_digests: BTreeMap::new(),
            original_digests: BTreeMap::new(),
            conflicts: Vec::new(),
            unstable: Vec::new(),
            complete: true,
            problems: Vec::new(),
        }
    }

    #[test]
    fn a_finding_survives_only_when_every_covering_closure_reports_it() {
        let mut got = collected(
            &[("//a:a", &["lib.py", "a.py"]), ("//b:b", &["lib.py"])],
            vec![
                finding("lib.py", "shared", "//a:a"),
                finding("lib.py", "shared", "//b:b"),
                finding("lib.py", "used_by_a", "//b:b"),
                finding("a.py", "own", "//a:a"),
            ],
        );
        keep_agreed_findings(&mut got);
        let names: Vec<&str> = got.initial.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(names, ["shared", "own"]);
        assert_eq!(got.terminal.len(), 2);
    }

    #[test]
    fn unused_requests_only_the_top_level_targets() {
        use super::super::test_support::Harness;
        let harness = Harness::new("unused-top-level");
        harness.write_source("app/BUILD.bazel", "");
        harness.query.script_stdout("//app:bin\n");
        let (code, out, err) = harness.run(&["unused", "//app:all"]);
        assert_eq!(code, 0, "{out}{err}");
        let expressions = harness.query.expressions.borrow();
        assert_eq!(
            expressions[0],
            "let p = \"//app:all\" in $p except deps(labels(deps, $p))"
        );
        let builds = harness.build_argv.borrow();
        assert!(builds[0].iter().any(|arg| arg == "//app:bin"), "{builds:?}");
    }
}
