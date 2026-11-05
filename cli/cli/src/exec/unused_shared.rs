//! Unused findings on a file that several targets analyze hold only when every one of them reports them.

use super::results::Collected;
use dx_output::DiagnosticEvent;

pub(crate) fn keep_shared(collected: &mut Collected) {
    let analyzed_by = &collected.analyzed_by;
    let (initial, terminal) = (&mut collected.initial, &mut collected.terminal);
    let all: Vec<DiagnosticEvent> = initial.iter().chain(terminal.iter()).cloned().collect();
    let keep = |event: &DiagnosticEvent| {
        let Some(path) = &event.path else {
            return true;
        };
        let key = (event.tool.clone(), path.clone());
        analyzed_by
            .iter()
            .filter(|(_, files)| files.contains(&key))
            .all(|(target, _)| {
                all.iter().any(|other| {
                    same_finding(event, other)
                        && other.origins.iter().any(|origin| &origin.target == target)
                })
            })
    };
    initial.retain(|event| keep(event));
    terminal.retain(|event| keep(event));
}

fn same_finding(a: &DiagnosticEvent, b: &DiagnosticEvent) -> bool {
    a.tool == b.tool
        && a.rule == b.rule
        && a.path == b.path
        && a.range == b.range
        && a.message == b.message
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_output::{Origin, Severity, Snapshot};
    use std::collections::{BTreeMap, BTreeSet};

    fn finding(path: &str, message: &str, targets: &[&str]) -> DiagnosticEvent {
        DiagnosticEvent {
            path: Some(path.to_owned()),
            range: Some((1, 2)),
            rule: Some("unused-function".to_owned()),
            snapshot: Snapshot::Initial,
            origins: targets
                .iter()
                .map(|target| Origin {
                    producer: "unused".to_owned(),
                    target: (*target).to_owned(),
                    configuration: None,
                })
                .collect(),
            ..DiagnosticEvent::new(Severity::Warning, "vulture", message)
        }
    }

    fn collected(by: &[(&str, &[&str])], initial: Vec<DiagnosticEvent>) -> Collected {
        let mut collected = Collected {
            tools: Vec::new(),
            ran: BTreeSet::new(),
            analyzed: BTreeSet::new(),
            analyzed_by: BTreeMap::new(),
            initial: Vec::new(),
            terminal: Vec::new(),
            changes: Vec::new(),
            terminal_digests: BTreeMap::new(),
            original_digests: BTreeMap::new(),
            conflicts: Vec::new(),
            unstable: Vec::new(),
            complete: true,
            problems: Vec::new(),
        };
        collected.analyzed_by = by
            .iter()
            .map(|(target, files)| {
                let files: BTreeSet<(String, String)> = files
                    .iter()
                    .map(|path| ("vulture".to_owned(), (*path).to_owned()))
                    .collect();
                ((*target).to_owned(), files)
            })
            .collect::<BTreeMap<_, _>>();
        collected.initial = initial;
        collected
    }

    #[test]
    fn finding_used_by_one_sharing_target_is_dropped() {
        let by: &[(&str, &[&str])] = &[("//a:one", &["lib.py"]), ("//a:two", &["lib.py"])];
        let mut shared = collected(by, vec![finding("lib.py", "f", &["//a:one"])]);
        keep_shared(&mut shared);
        assert!(shared.initial.is_empty());
    }

    #[test]
    fn finding_reported_by_every_sharing_target_is_kept() {
        let by: &[(&str, &[&str])] = &[("//a:one", &["lib.py"]), ("//a:two", &["lib.py"])];
        let mut shared = collected(
            by,
            vec![
                finding("lib.py", "h", &["//a:one"]),
                finding("lib.py", "h", &["//a:two"]),
            ],
        );
        keep_shared(&mut shared);
        assert_eq!(shared.initial.len(), 2);
    }

    #[test]
    fn finding_in_a_file_one_target_analyzed_is_kept() {
        let by: &[(&str, &[&str])] = &[("//a:one", &["lib.py"]), ("//a:two", &["other.py"])];
        let mut shared = collected(by, vec![finding("lib.py", "f", &["//a:one"])]);
        keep_shared(&mut shared);
        assert_eq!(shared.initial.len(), 1);
    }
}
