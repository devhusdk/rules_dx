use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::plan::OUTPUT_GROUP;
use dx_output::{DiagnosticEvent, Severity, Snapshot};
use quality_result::{decode_validated, proto};

use super::common::{
    apply_to_bytes, collect_targets, FileChange, REASON_CONFLICTING_EDITS,
    REASON_CONFLICTING_ORIGINAL_DIGEST, REASON_CONFLICTING_TERMINAL_DIGEST,
};

struct Candidate {
    change: FileChange,
    producers: Vec<String>,
}

#[derive(Default)]
struct Staged {
    tools: Vec<String>,
    initial: Vec<DiagnosticEvent>,
    terminal: Vec<DiagnosticEvent>,
    changes: Vec<Candidate>,
    digests: Vec<(String, [u8; 32], String)>,
}

impl Staged {
    fn absorb(&mut self, other: Staged) {
        self.tools.extend(other.tools);
        self.initial.extend(other.initial);
        self.terminal.extend(other.terminal);
        self.changes.extend(other.changes);
        self.digests.extend(other.digests);
    }
}

/// One workspace path whose producers disagree, kept out of every projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub(crate) path: String,
    pub(crate) reason: &'static str,
    pub(crate) producers: Vec<String>,
}

/// Merges same-path candidates into one change or one explicit conflict.
fn reconcile_changes(
    workspace: &Path,
    candidates: Vec<Candidate>,
) -> (Vec<FileChange>, Vec<Conflict>) {
    let mut by_path: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for candidate in candidates {
        by_path
            .entry(candidate.change.path.clone())
            .or_default()
            .push(candidate);
    }
    let mut changes = Vec::with_capacity(by_path.len());
    let mut conflicts = Vec::new();
    for (path, mut group) in by_path {
        group.sort_by(|a, b| {
            a.producers
                .cmp(&b.producers)
                .then_with(|| a.change.original_digest.cmp(&b.change.original_digest))
                .then_with(|| a.change.edits.cmp(&b.change.edits))
        });
        if group.len() == 1 {
            if let Some(candidate) = group.pop() {
                changes.push(candidate.change);
            }
            continue;
        }
        let producers = producers_of(&group);
        if group
            .iter()
            .any(|candidate| candidate.change.original_digest != group[0].change.original_digest)
        {
            conflicts.push(Conflict {
                path,
                reason: REASON_CONFLICTING_ORIGINAL_DIGEST,
                producers,
            });
            continue;
        }
        let agreed = match terminal_candidates(workspace, &path, &group) {
            Some(terminals) => terminals.len() == 1,
            None => group
                .iter()
                .all(|candidate| candidate.change.edits == group[0].change.edits),
        };
        if agreed {
            if let Some(candidate) = group.pop() {
                changes.push(candidate.change);
            }
        } else {
            conflicts.push(Conflict {
                path,
                reason: REASON_CONFLICTING_EDITS,
                producers,
            });
        }
    }
    (changes, conflicts)
}

fn producers_of(group: &[Candidate]) -> Vec<String> {
    group
        .iter()
        .flat_map(|candidate| candidate.producers.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn terminal_candidates(
    workspace: &Path,
    path: &str,
    group: &[Candidate],
) -> Option<BTreeSet<Vec<u8>>> {
    let original = std::fs::read(workspace.join(path)).ok()?;
    if dx_digest::blake3(&original) != group[0].change.original_digest {
        return None;
    }
    group
        .iter()
        .map(|candidate| apply_to_bytes(&original, &candidate.change.edits))
        .collect()
}

fn reconcile_digests(
    digests: Vec<(String, [u8; 32], String)>,
) -> (BTreeMap<String, [u8; 32]>, Vec<Conflict>) {
    let mut by_path: BTreeMap<String, Vec<([u8; 32], String)>> = BTreeMap::new();
    for (path, digest, producer) in digests {
        by_path.entry(path).or_default().push((digest, producer));
    }
    let mut agreed = BTreeMap::new();
    let mut conflicts = Vec::new();
    for (path, values) in by_path {
        let unique: BTreeSet<[u8; 32]> = values.iter().map(|(digest, _)| *digest).collect();
        if unique.len() == 1 {
            if let Some(digest) = unique.into_iter().next() {
                agreed.insert(path, digest);
            }
            continue;
        }
        let producers = values
            .into_iter()
            .map(|(_, producer)| producer)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        conflicts.push(Conflict {
            path,
            reason: REASON_CONFLICTING_TERMINAL_DIGEST,
            producers,
        });
    }
    (agreed, conflicts)
}

/// Decodes one result artifact into the collected shape, or None when it is not one.
fn stage_artifact(bytes: &[u8]) -> Option<Staged> {
    stage_result(&decode_validated(bytes).ok()?)
}

/// Maps one decoded result, or None when a field is outside what the report can carry.
fn stage_result(result: &proto::QualityResult) -> Option<Staged> {
    let mut staged = Staged::default();
    for stage in &result.stages {
        staged.tools.push(stage.tool_id.clone());
    }
    for snapshot in &result.terminal_snapshot {
        let digest: [u8; 32] = snapshot.digest.as_slice().try_into().ok()?;
        staged
            .digests
            .push((snapshot.path.clone(), digest, result.producer.clone()));
    }
    for diagnostic in &result.initial_diagnostics {
        staged
            .initial
            .push(map_diagnostic(diagnostic, Snapshot::Initial)?);
    }
    for diagnostic in &result.terminal_diagnostics {
        staged
            .terminal
            .push(map_diagnostic(diagnostic, Snapshot::Terminal)?);
    }
    for file in &result.replacements {
        staged.changes.push(Candidate {
            change: map_change(file)?,
            producers: vec![result.producer.clone()],
        });
    }
    Some(staged)
}

pub(crate) struct Collected {
    pub(crate) tools: Vec<String>,
    pub(crate) initial: Vec<DiagnosticEvent>,
    pub(crate) terminal: Vec<DiagnosticEvent>,
    pub(crate) changes: Vec<FileChange>,
    pub(crate) conflicts: Vec<Conflict>,
    pub(crate) terminal_digests: BTreeMap<String, [u8; 32]>,
    pub(crate) complete: bool,
}

fn map_severity(value: i32) -> Option<Severity> {
    match proto::Severity::try_from(value).ok()? {
        proto::Severity::Unspecified => None,
        proto::Severity::Info => Some(Severity::Info),
        proto::Severity::Warning => Some(Severity::Warning),
        proto::Severity::Error => Some(Severity::Error),
    }
}

fn map_diagnostic(diagnostic: &proto::Diagnostic, snapshot: Snapshot) -> Option<DiagnosticEvent> {
    let range = match (diagnostic.start_byte, diagnostic.end_byte) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        _ => return None,
    };
    let path = (!diagnostic.path.is_empty()).then(|| diagnostic.path.clone());
    if path.is_none() && range.is_some() {
        return None;
    }
    Some(DiagnosticEvent {
        severity: map_severity(diagnostic.severity)?,
        tool: diagnostic.tool_id.clone(),
        message: diagnostic.message.clone(),
        rule: (!diagnostic.rule_id.is_empty()).then(|| diagnostic.rule_id.clone()),
        path,
        range,
        snapshot,
        fixable: diagnostic.fixable,
        resolution: None,
    })
}

fn map_change(change: &proto::FileEdits) -> Option<FileChange> {
    let original_digest: [u8; 32] = change.original_digest.as_slice().try_into().ok()?;
    let mut edits = Vec::with_capacity(change.edits.len());
    for edit in &change.edits {
        edits.push((
            edit.start_byte,
            edit.end_byte,
            String::from_utf8(edit.replacement.clone()).ok()?,
        ));
    }
    Some(FileChange {
        path: change.path.clone(),
        original_digest,
        edits,
    })
}

pub(crate) fn collect_results(bep: &Path, workspace: &Path) -> Result<Collected, (String, String)> {
    collect_results_in(bep, OUTPUT_GROUP, workspace)
}

pub(crate) fn collect_results_in(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<Collected, (String, String)> {
    let targets = collect_targets(bep, group, workspace)?;
    let mut tools = BTreeSet::new();
    let mut initial = Vec::new();
    let mut terminal = Vec::new();
    let mut changes = Vec::new();
    let mut terminal_digests = BTreeMap::new();
    let mut complete = true;
    for target in &targets {
        if !target.success {
            complete = false;
            continue;
        }
        let mut staged = Staged::default();
        let mut staged_ok = true;
        for artifact in &target.artifacts {
            let Some(mut part) = stage_artifact(&artifact.bytes) else {
                staged_ok = false;
                break;
            };
            for candidate in &mut part.changes {
                candidate.producers.push(target.label.clone());
            }
            staged.absorb(part);
        }
        if !staged_ok {
            complete = false;
            continue;
        }
        tools.extend(staged.tools);
        initial.extend(staged.initial);
        terminal.extend(staged.terminal);
        changes.extend(staged.changes);
        terminal_digests.extend(staged.digests);
    }
    let (changes, mut conflicts) = reconcile_changes(workspace, changes);
    let (terminal_digests, digest_conflicts) = reconcile_digests(terminal_digests);
    for conflict in digest_conflicts {
        if !conflicts
            .iter()
            .any(|existing| existing.path == conflict.path)
        {
            conflicts.push(conflict);
        }
    }
    conflicts.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let conflicted: BTreeSet<&str> = conflicts
        .iter()
        .map(|conflict| conflict.path.as_str())
        .collect();
    let terminal_digests = terminal_digests
        .into_iter()
        .filter(|(path, _)| !conflicted.contains(path.as_str()))
        .collect();
    Ok(Collected {
        tools: tools.into_iter().collect(),
        initial,
        terminal,
        changes,
        conflicts,
        terminal_digests,
        complete,
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::exec::common::CODE_INVALID_BEP;
    use quality_result::{encode_validated, proto};

    #[test]
    fn severity_mapping_covers_all_arms() {
        assert!(map_severity(proto::Severity::Unspecified as i32).is_none());
        assert!(matches!(
            map_severity(proto::Severity::Info as i32),
            Some(Severity::Info)
        ));
        assert!(matches!(
            map_severity(proto::Severity::Warning as i32),
            Some(Severity::Warning)
        ));
        assert!(matches!(
            map_severity(proto::Severity::Error as i32),
            Some(Severity::Error)
        ));
        assert!(map_severity(99).is_none());
    }

    #[test]
    fn diagnostic_mapping_rejects_bad_shapes() {
        let base = Harness::diagnostic("m", false);
        let mut partial = base.clone();
        partial.end_byte = None;
        assert!(map_diagnostic(&partial, Snapshot::Initial).is_none());
        let mut pathless = base.clone();
        pathless.path = String::new();
        assert!(map_diagnostic(&pathless, Snapshot::Initial).is_none());
        assert!(map_diagnostic(&pathless, Snapshot::Terminal).is_none());
        let mut bare = pathless.clone();
        bare.start_byte = None;
        bare.end_byte = None;
        assert!(map_diagnostic(&bare, Snapshot::Terminal).is_some());
    }

    #[test]
    fn staging_rejects_results_the_report_cannot_carry() {
        let harness = Harness::new("stage-reject");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = harness.valid_result(
            vec![Harness::diagnostic("unused", false)],
            vec![harness.replacement(b"y")],
        );
        let mut result = decode_validated(&bytes).expect("valid result");
        result.terminal_diagnostics = result.initial_diagnostics.clone();
        assert!(stage_artifact(&bytes).is_some(), "a valid artifact stages");
        assert!(stage_result(&result).is_some(), "a valid result stages");
        assert!(stage_artifact(b"not-a-validated-result").is_none());

        let mut short_snapshot = result.clone();
        short_snapshot.terminal_snapshot[0].digest.pop();
        let mut no_severity = result.clone();
        no_severity.initial_diagnostics[0].severity = proto::Severity::Unspecified as i32;
        let mut no_end = result.clone();
        no_end.terminal_diagnostics[0].end_byte = None;
        let mut short_original = result.clone();
        short_original.replacements[0].original_digest.pop();
        for broken in [short_snapshot, no_severity, no_end, short_original] {
            assert!(stage_result(&broken).is_none());
            assert!(
                encode_validated(&broken).is_err(),
                "a producer cannot send what staging rejects"
            );
        }
    }

    #[test]
    fn staging_rejects_non_utf8_replacements() {
        let harness = Harness::new("stage-bytes");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = harness.valid_result(vec![], vec![harness.replacement(b"y")]);
        let mut result = decode_validated(&bytes).expect("valid result");
        result.replacements[0].edits[0].replacement = vec![0xFF];
        assert!(
            stage_result(&result).is_none(),
            "a change whose replacement is not text never reaches the report"
        );
        assert!(
            encode_validated(&result).is_err(),
            "a producer cannot send what staging rejects"
        );
    }

    #[test]
    fn invalid_output_group_rejected() {
        let dir = temp_dir("group-tmp");
        let bep = dir.path().join("empty.json");
        std::fs::write(&bep, "").expect("bep");
        let Err((code, message)) = collect_results_in(&bep, "", dir.path()) else {
            panic!("empty output group must fail"); // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        };
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(message.contains("invalid BEP config"));
    }
}
