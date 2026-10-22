use super::common::{
    apply_to_bytes, read_verified, FileChange, SourceRead, REASON_CONFLICTING_CANDIDATES,
    REASON_CONFLICTING_DIGESTS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) struct Producer {
    pub(crate) label: String,
    pub(crate) tools: Vec<String>,
    pub(crate) changes: Vec<FileChange>,
    pub(crate) digests: Vec<(String, [u8; 32])>,
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
pub(crate) fn reconcile(workspace: &Path, producers: Vec<Producer>) -> Reconciled {
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
    };
    for path in paths {
        let path_changes = changes.get(&path).map(Vec::as_slice).unwrap_or(&[]);
        let path_digests = digests.get(&path).map(Vec::as_slice).unwrap_or(&[]);
        let originals: BTreeSet<[u8; 32]> = path_changes
            .iter()
            .map(|(_, change)| change.original_digest)
            .collect();
        let terminals: BTreeSet<[u8; 32]> = path_digests.iter().map(|(_, d)| *d).collect();
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
            out.digests.insert(path, digest);
        }
    }
    out
}
