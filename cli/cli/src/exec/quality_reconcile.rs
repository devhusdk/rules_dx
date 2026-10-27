use super::common::{
    apply_to_bytes, read_verified, FileChange, SourceRead, REASON_CONFLICTING_CANDIDATES,
    REASON_CONFLICTING_DIGESTS,
};
use dx_output::{DiagnosticEvent, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) struct Producer {
    pub(crate) label: String,
    pub(crate) tools: Vec<String>,
    pub(crate) changes: Vec<FileChange>,
    pub(crate) digests: Vec<(String, [u8; 32])>,
    pub(crate) originals: Vec<(String, [u8; 32])>,
    pub(crate) terminal: Vec<DiagnosticEvent>,
}

pub(crate) struct Conflict {
    pub(crate) path: String,
    pub(crate) reason: &'static str,
    pub(crate) producers: Vec<String>,
}

pub(crate) struct Reconciled {
    pub(crate) changes: Vec<FileChange>,
    pub(crate) digests: BTreeMap<String, [u8; 32]>,
    pub(crate) conflicts: Vec<Conflict>,
    pub(crate) terminal: Vec<DiagnosticEvent>,
}

fn identity(producer: &Producer) -> String {
    let mut tools: Vec<&str> = producer.tools.iter().map(String::as_str).collect();
    tools.sort_unstable();
    tools.dedup();
    format!("{} [{}]", producer.label, tools.join(", "))
}

fn candidate_key(source: &SourceRead, change: &FileChange) -> Result<Option<Vec<u8>>, ()> {
    match source {
        SourceRead::Bytes(original) => Ok(apply_to_bytes(original, &change.edits)),
        _ => Err(()),
    }
}

fn candidates_agree(workspace: &Path, path: &str, changes: &[(usize, &FileChange)]) -> bool {
    let first = changes[0].1;
    let source = read_verified(workspace, path, &first.original_digest);
    let first_key = candidate_key(&source, first);
    changes.iter().all(|(_, change)| match &first_key {
        Ok(Some(bytes)) => {
            matches!(candidate_key(&source, change), Ok(Some(other)) if &other == bytes)
        }
        _ => change.edits == first.edits,
    })
}

/// Merges per-producer results, rejecting paths whose producers disagree.
///
/// A producer that has no change for a path, saw the same original bytes the
/// editors did, and left them untouched is an observer: it does not conflict
/// with the editors. Its terminal findings on that path describe the original
/// bytes, so they are retagged `Initial` when an editor rewrites the file.
pub(crate) fn reconcile(workspace: &Path, mut producers: Vec<Producer>) -> Reconciled {
    let mut changes: BTreeMap<String, Vec<(usize, &FileChange)>> = BTreeMap::new();
    let mut digests: BTreeMap<String, Vec<(usize, [u8; 32])>> = BTreeMap::new();
    for (index, producer) in producers.iter().enumerate() {
        for change in &producer.changes {
            changes
                .entry(change.path.clone())
                .or_default()
                .push((index, change));
        }
        for (path, digest) in &producer.digests {
            digests
                .entry(path.clone())
                .or_default()
                .push((index, *digest));
        }
    }
    let paths: BTreeSet<String> = changes.keys().chain(digests.keys()).cloned().collect();
    let mut out = Reconciled {
        changes: Vec::new(),
        digests: BTreeMap::new(),
        conflicts: Vec::new(),
        terminal: Vec::new(),
    };
    let mut retag: BTreeSet<(usize, String)> = BTreeSet::new();
    for path in paths {
        let path_changes = changes.get(&path).map(Vec::as_slice).unwrap_or(&[]);
        let path_digests = digests.get(&path).map(Vec::as_slice).unwrap_or(&[]);
        let originals: BTreeSet<[u8; 32]> = path_changes
            .iter()
            .map(|(_, change)| change.original_digest)
            .collect();
        let observer = |index: usize, digest: &[u8; 32]| {
            !path_changes.is_empty()
                && !path_changes.iter().any(|(held, _)| *held == index)
                && producers[index]
                    .originals
                    .iter()
                    .any(|(p, o)| *p == path && o == digest && originals.contains(o))
        };
        let observers: BTreeSet<usize> = path_digests
            .iter()
            .filter(|(index, digest)| observer(*index, digest))
            .map(|(index, _)| *index)
            .collect();
        let mut terminals: BTreeSet<[u8; 32]> = path_digests
            .iter()
            .filter(|(index, _)| !observers.contains(index))
            .map(|(_, d)| *d)
            .collect();
        if terminals.is_empty() {
            terminals = path_digests.iter().map(|(_, d)| *d).collect();
        }
        let reason = if originals.len() > 1 || terminals.len() > 1 {
            Some(REASON_CONFLICTING_DIGESTS)
        } else if !path_changes.is_empty() && !candidates_agree(workspace, &path, path_changes) {
            Some(REASON_CONFLICTING_CANDIDATES)
        } else {
            None
        };
        if let Some(reason) = reason {
            let involved: BTreeSet<usize> = path_changes
                .iter()
                .map(|(index, _)| *index)
                .chain(path_digests.iter().map(|(index, _)| *index))
                .collect();
            let names: BTreeSet<String> = involved
                .into_iter()
                .map(|index| identity(&producers[index]))
                .collect();
            out.conflicts.push(Conflict {
                path,
                reason,
                producers: names.into_iter().collect(),
            });
            continue;
        }
        if let Some(chosen) = path_changes
            .iter()
            .map(|(_, change)| *change)
            .min_by(|a, b| a.edits.cmp(&b.edits))
        {
            out.changes.push(chosen.clone());
        }
        if let Some(digest) = terminals.into_iter().next() {
            for (index, held) in path_digests {
                if observers.contains(index) && *held != digest {
                    retag.insert((*index, path.clone()));
                }
            }
            out.digests.insert(path, digest);
        }
    }
    for (index, producer) in producers.iter_mut().enumerate() {
        for mut finding in std::mem::take(&mut producer.terminal) {
            if let Some(path) = &finding.path {
                if retag.contains(&(index, path.clone())) {
                    finding.snapshot = Snapshot::Initial;
                }
            }
            out.terminal.push(finding);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL: [u8; 32] = [1; 32];
    const FIXED: [u8; 32] = [2; 32];

    fn change() -> FileChange {
        FileChange {
            path: "a.py".to_owned(),
            original_digest: ORIGINAL,
            edits: vec![(0, 1, "x".to_owned())],
        }
    }

    fn finding(snapshot: Snapshot) -> DiagnosticEvent {
        DiagnosticEvent {
            path: Some("a.py".to_owned()),
            snapshot,
            ..DiagnosticEvent::new(dx_output::Severity::Warning, "t".to_owned(), "m".to_owned())
        }
    }

    fn editor() -> Producer {
        Producer {
            label: "//core".to_owned(),
            tools: vec!["ruff".to_owned()],
            changes: vec![change()],
            digests: vec![("a.py".to_owned(), FIXED)],
            originals: vec![("a.py".to_owned(), ORIGINAL)],
            terminal: vec![finding(Snapshot::Terminal)],
        }
    }

    fn observer(original: [u8; 32], terminal: [u8; 32]) -> Producer {
        Producer {
            label: "//py".to_owned(),
            tools: vec!["pydoclint".to_owned()],
            changes: vec![],
            digests: vec![("a.py".to_owned(), terminal)],
            originals: vec![("a.py".to_owned(), original)],
            terminal: vec![finding(Snapshot::Terminal)],
        }
    }

    #[test]
    fn observer_does_not_conflict_in_either_order() {
        for editor_first in [true, false] {
            let mut producers = vec![editor(), observer(ORIGINAL, ORIGINAL)];
            if !editor_first {
                producers.reverse();
            }
            let out = reconcile(Path::new("."), producers);
            assert!(out.conflicts.is_empty());
            assert_eq!(out.changes.len(), 1);
            assert_eq!(out.digests["a.py"], FIXED);
            let initial = out
                .terminal
                .iter()
                .filter(|f| f.snapshot == Snapshot::Initial)
                .count();
            assert_eq!((initial, out.terminal.len()), (1, 2));
        }
    }

    #[test]
    fn stale_observer_conflicts() {
        let out = reconcile(Path::new("."), vec![editor(), observer([9; 32], [9; 32])]);
        assert_eq!(out.conflicts.len(), 1);
        assert_eq!(out.conflicts[0].reason, REASON_CONFLICTING_DIGESTS);
    }

    #[test]
    fn producer_without_original_conflicts() {
        let mut other = observer(ORIGINAL, ORIGINAL);
        other.originals.clear();
        let out = reconcile(Path::new("."), vec![editor(), other]);
        assert_eq!(out.conflicts.len(), 1);
    }

    #[test]
    fn two_observers_and_one_editor() {
        let producers = vec![
            observer(ORIGINAL, ORIGINAL),
            editor(),
            observer(ORIGINAL, ORIGINAL),
        ];
        let out = reconcile(Path::new("."), producers);
        assert!(out.conflicts.is_empty());
        assert_eq!(out.digests["a.py"], FIXED);
    }

    #[test]
    fn editors_with_different_terminals_conflict() {
        let mut other = editor();
        other.digests = vec![("a.py".to_owned(), [3; 32])];
        let out = reconcile(Path::new("."), vec![editor(), other]);
        assert_eq!(out.conflicts.len(), 1);
    }
}
