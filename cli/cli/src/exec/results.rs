use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::plan::OUTPUT_GROUP;
use dx_output::{DiagnosticEvent, Severity, Snapshot};
use quality_result::{decode_validated, proto};

use super::common::{
    apply_to_bytes, collect_targets, FileChange, REASON_CONFLICTING_EDITS,
    REASON_CONFLICTING_ORIGINAL_DIGEST, REASON_CONFLICTING_TERMINAL_DIGEST,
};

#[derive(Clone)]
struct Candidate {
    change: FileChange,
    producers: BTreeSet<String>,
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

impl Conflict {
    pub(crate) fn describe(&self) -> String {
        format!(
            "Conflict: {} ({}; producers: {})",
            self.path,
            self.reason,
            self.producers.join(", ")
        )
    }
}

/// Merges same-path candidates and terminal digests into one change or one explicit conflict.
fn reconcile_changes(
    workspace: &Path,
    candidates: Vec<Candidate>,
    digests: Vec<(String, [u8; 32], String)>,
) -> (Vec<FileChange>, BTreeMap<String, [u8; 32]>, Vec<Conflict>) {
    let mut candidates_by_path: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for candidate in candidates {
        candidates_by_path
            .entry(candidate.change.path.clone())
            .or_default()
            .push(candidate);
    }
    let mut digests_by_path: BTreeMap<String, Vec<([u8; 32], String)>> = BTreeMap::new();
    for (path, digest, producer) in digests {
        digests_by_path
            .entry(path)
            .or_default()
            .push((digest, producer));
    }
    let mut paths: BTreeSet<String> = candidates_by_path.keys().cloned().collect();
    paths.extend(digests_by_path.keys().cloned());
    let mut changes = Vec::with_capacity(candidates_by_path.len());
    let mut terminal_digests = BTreeMap::new();
    let mut conflicts = Vec::new();
    for path in paths {
        let group = candidates_by_path.remove(&path).unwrap_or_default();
        let observed = digests_by_path.remove(&path).unwrap_or_default();
        let mut producers: BTreeSet<String> = group
            .iter()
            .flat_map(|candidate| candidate.producers.iter().cloned())
            .collect();
        producers.extend(observed.iter().map(|(_, producer)| producer.clone()));
        let unique: BTreeSet<[u8; 32]> = observed.iter().map(|(digest, _)| *digest).collect();
        if unique.len() > 1 {
            conflicts.push(Conflict {
                path,
                reason: REASON_CONFLICTING_TERMINAL_DIGEST,
                producers: producers.into_iter().collect(),
            });
            continue;
        }
        let agreed = unique.into_iter().next();
        if group.is_empty() {
            if let Some(digest) = agreed {
                terminal_digests.insert(path, digest);
            }
            continue;
        }
        let originals: BTreeSet<[u8; 32]> = group
            .iter()
            .map(|candidate| candidate.change.original_digest)
            .collect();
        if originals.len() > 1 {
            conflicts.push(Conflict {
                path,
                reason: REASON_CONFLICTING_ORIGINAL_DIGEST,
                producers: producers.into_iter().collect(),
            });
            continue;
        }
        let original_digest = group[0].change.original_digest;
        match terminal_candidates(workspace, &path, original_digest, &group) {
            Some(terminals) if terminals.len() > 1 => {
                conflicts.push(Conflict {
                    path,
                    reason: REASON_CONFLICTING_EDITS,
                    producers: producers.into_iter().collect(),
                });
            }
            Some(_) => {
                if let Some(candidate) = choose_candidate(group) {
                    changes.push(candidate.change);
                    if let Some(digest) = agreed {
                        terminal_digests.insert(path, digest);
                    }
                }
            }
            None => {
                let distinct: BTreeSet<&Vec<(u64, u64, String)>> = group
                    .iter()
                    .map(|candidate| &candidate.change.edits)
                    .collect();
                if distinct.len() > 1 {
                    conflicts.push(Conflict {
                        path,
                        reason: REASON_CONFLICTING_EDITS,
                        producers: producers.into_iter().collect(),
                    });
                } else if let Some(candidate) = choose_candidate(group) {
                    changes.push(candidate.change);
                    if let Some(digest) = agreed {
                        terminal_digests.insert(path, digest);
                    }
                }
            }
        }
    }
    conflicts.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    (changes, terminal_digests, conflicts)
}

fn choose_candidate(group: Vec<Candidate>) -> Option<Candidate> {
    group.into_iter().min_by(|a, b| {
        a.change
            .edits
            .cmp(&b.change.edits)
            .then_with(|| a.producers.cmp(&b.producers))
    })
}

fn terminal_candidates(
    workspace: &Path,
    path: &str,
    original_digest: [u8; 32],
    group: &[Candidate],
) -> Option<BTreeSet<Vec<u8>>> {
    let original = std::fs::read(workspace.join(path)).ok()?;
    if dx_digest::blake3(&original) != original_digest {
        return None;
    }
    group
        .iter()
        .map(|candidate| apply_to_bytes(&original, &candidate.change.edits))
        .collect()
}

/// Decodes one result artifact into the collected shape, or None when it is not one.
fn stage_artifact(bytes: &[u8], target: &str) -> Option<Staged> {
    stage_result(&decode_validated(bytes).ok()?, target)
}

/// Maps one decoded result, or None when a field is outside what the report can carry.
fn stage_result(result: &proto::QualityResult, target: &str) -> Option<Staged> {
    let mut staged = Staged::default();
    let mut producers: BTreeSet<String> = BTreeSet::new();
    producers.insert(result.producer.clone());
    if !target.is_empty() && target != result.producer {
        producers.insert(target.to_owned());
    }
    for stage in &result.stages {
        staged.tools.push(stage.tool_id.clone());
    }
    for snapshot in &result.terminal_snapshot {
        let digest: [u8; 32] = snapshot.digest.as_slice().try_into().ok()?;
        for producer in &producers {
            staged
                .digests
                .push((snapshot.path.clone(), digest, producer.clone()));
        }
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
            producers: producers.clone(),
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
    let mut candidates = Vec::new();
    let mut digests = Vec::new();
    let mut complete = true;
    for target in &targets {
        if !target.success {
            complete = false;
            continue;
        }
        let mut staged = Staged::default();
        let mut staged_ok = true;
        for artifact in &target.artifacts {
            let Some(part) = stage_artifact(&artifact.bytes, &target.label) else {
                staged_ok = false;
                break;
            };
            staged.absorb(part);
        }
        if !staged_ok {
            complete = false;
            continue;
        }
        tools.extend(staged.tools);
        initial.extend(staged.initial);
        terminal.extend(staged.terminal);
        candidates.extend(staged.changes);
        digests.extend(staged.digests);
    }
    let (changes, terminal_digests, conflicts) = reconcile_changes(workspace, candidates, digests);
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
        assert!(
            stage_artifact(&bytes, "//test:corpus").is_some(),
            "a valid artifact stages"
        );
        assert!(
            stage_result(&result, "//test:corpus").is_some(),
            "a valid result stages"
        );
        assert!(stage_artifact(b"not-a-validated-result", "//test:corpus").is_none());

        let mut short_snapshot = result.clone();
        short_snapshot.terminal_snapshot[0].digest.pop();
        let mut no_severity = result.clone();
        no_severity.initial_diagnostics[0].severity = proto::Severity::Unspecified as i32;
        let mut no_end = result.clone();
        no_end.terminal_diagnostics[0].end_byte = None;
        let mut short_original = result.clone();
        short_original.replacements[0].original_digest.pop();
        for broken in [short_snapshot, no_severity, no_end, short_original] {
            assert!(stage_result(&broken, "//test:corpus").is_none());
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
            stage_result(&result, "//test:corpus").is_none(),
            "a change whose replacement is not text never reaches the report"
        );
        assert!(
            encode_validated(&result).is_err(),
            "a producer cannot send what staging rejects"
        );
    }

    fn candidate(original: &[u8], edits: Vec<(u64, u64, &str)>, producers: &[&str]) -> Candidate {
        Candidate {
            change: FileChange {
                path: "src/a.py".to_owned(),
                original_digest: dx_digest::blake3(original),
                edits: edits
                    .into_iter()
                    .map(|(start, end, text)| (start, end, text.to_owned()))
                    .collect(),
            },
            producers: producers.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn full(original: &[u8], terminal: &str, producers: &[&str]) -> Candidate {
        candidate(
            original,
            vec![(0, original.len() as u64, terminal)],
            producers,
        )
    }

    fn reconcile_fixture(
        name: &str,
        body: &str,
        candidates: Vec<Candidate>,
        digests: Vec<(&str, &[u8], &str)>,
    ) -> (Vec<FileChange>, BTreeMap<String, [u8; 32]>, Vec<Conflict>) {
        let dir = temp_dir(name);
        std::fs::create_dir_all(dir.path().join("src")).expect("dirs");
        std::fs::write(dir.path().join("src/a.py"), body).expect("source");
        let observed = digests
            .into_iter()
            .map(|(path, bytes, producer)| {
                (
                    path.to_owned(),
                    dx_digest::blake3(bytes),
                    producer.to_owned(),
                )
            })
            .collect();
        reconcile_changes(dir.path(), candidates, observed)
    }

    #[test]
    fn identical_candidates_merge_into_one_change() {
        let original = b"x = 1\n";
        let terminal = b"y = 1\n";
        let first = full(original, "y = 1\n", &["//a:lint"]);
        let second = full(original, "y = 1\n", &["//b:format"]);
        let (changes, digests, conflicts) = reconcile_fixture(
            "reconcile-identical",
            "x = 1\n",
            vec![first, second],
            vec![
                ("src/a.py", terminal, "//a:lint"),
                ("src/a.py", terminal, "//b:format"),
            ],
        );
        assert!(conflicts.is_empty());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/a.py");
        assert_eq!(digests.get("src/a.py"), Some(&dx_digest::blake3(terminal)));
    }

    #[test]
    fn equivalent_terminal_bytes_merge_despite_different_edits() {
        let original = b"ab\n";
        let terminal = b"cd\n";
        let whole = full(original, "cd\n", &["//a:lint"]);
        let split = candidate(original, vec![(0, 1, "c"), (1, 2, "d")], &["//b:format"]);
        let (changes, digests, conflicts) = reconcile_fixture(
            "reconcile-equivalent",
            "ab\n",
            vec![whole, split],
            vec![
                ("src/a.py", terminal, "//a:lint"),
                ("src/a.py", terminal, "//b:format"),
            ],
        );
        assert!(conflicts.is_empty());
        assert_eq!(changes.len(), 1);
        assert_eq!(
            apply_to_bytes(original, &changes[0].edits),
            Some(terminal.to_vec())
        );
        assert_eq!(digests.get("src/a.py"), Some(&dx_digest::blake3(terminal)));
    }

    #[test]
    fn disagreeing_terminal_candidates_conflict_with_producers() {
        let original = b"ab\n";
        let first = full(original, "cd\n", &["//a:lint"]);
        let second = full(original, "ce\n", &["//b:format"]);
        let (changes, digests, conflicts) =
            reconcile_fixture("reconcile-edits", "ab\n", vec![first, second], vec![]);
        assert!(changes.is_empty());
        assert!(digests.is_empty());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "src/a.py");
        assert_eq!(conflicts[0].reason, REASON_CONFLICTING_EDITS);
        assert_eq!(
            conflicts[0].producers,
            vec!["//a:lint".to_owned(), "//b:format".to_owned()]
        );
    }

    #[test]
    fn disagreeing_original_digests_conflict() {
        let first = full(b"ab\n", "cd\n", &["//a:lint"]);
        let second = full(b"xy\n", "cd\n", &["//b:format"]);
        let (changes, _, conflicts) =
            reconcile_fixture("reconcile-original", "ab\n", vec![first, second], vec![]);
        assert!(changes.is_empty());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].reason, REASON_CONFLICTING_ORIGINAL_DIGEST);
    }

    #[test]
    fn disagreeing_terminal_digests_conflict_and_drop_the_change() {
        let original = b"x = 1\n";
        let change = full(original, "y = 1\n", &["//a:lint"]);
        let other = full(original, "y = 1\n", &["//b:format"]);
        let (changes, digests, conflicts) = reconcile_fixture(
            "reconcile-terminal-digest",
            "x = 1\n",
            vec![change, other],
            vec![
                ("src/a.py", b"y = 1\n".as_slice(), "//a:lint"),
                ("src/a.py", b"z = 2\n".as_slice(), "//b:format"),
            ],
        );
        assert!(changes.is_empty());
        assert!(digests.is_empty());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].reason, REASON_CONFLICTING_TERMINAL_DIGEST);
    }

    #[test]
    fn missing_source_still_merges_identical_edits_and_rejects_others() {
        let first = full(b"ab\n", "cd\n", &["//a:lint"]);
        let same = full(b"ab\n", "cd\n", &["//b:format"]);
        let (changes, _, conflicts) = reconcile_fixture(
            "reconcile-missing-same",
            "",
            vec![first.clone(), same],
            vec![],
        );
        assert!(conflicts.is_empty());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].edits, first.change.edits);
        let other = full(b"ab\n", "ce\n", &["//b:format"]);
        let (changes, _, conflicts) = reconcile_fixture(
            "reconcile-missing-different",
            "",
            vec![first, other],
            vec![],
        );
        assert!(changes.is_empty());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].reason, REASON_CONFLICTING_EDITS);
    }

    #[test]
    fn digest_only_paths_agree_or_conflict_without_changes() {
        let dir = temp_dir("reconcile-digest-only");
        let agreed = reconcile_changes(
            dir.path(),
            vec![],
            vec![
                ("src/a.py".to_owned(), [1u8; 32], "//a:lint".to_owned()),
                ("src/a.py".to_owned(), [1u8; 32], "//b:format".to_owned()),
            ],
        );
        assert_eq!(agreed.0.len(), 0);
        assert_eq!(agreed.2.len(), 0);
        assert_eq!(agreed.1.get("src/a.py"), Some(&[1u8; 32]));
        let disagreed = reconcile_changes(
            dir.path(),
            vec![],
            vec![
                ("src/a.py".to_owned(), [1u8; 32], "//a:lint".to_owned()),
                ("src/a.py".to_owned(), [2u8; 32], "//b:format".to_owned()),
            ],
        );
        assert_eq!(disagreed.2.len(), 1);
        assert_eq!(disagreed.2[0].reason, REASON_CONFLICTING_TERMINAL_DIGEST);
        assert!(disagreed.1.is_empty());
    }

    #[test]
    fn candidate_order_never_changes_the_decision() {
        let original = b"ab\n";
        let first = full(original, "cd\n", &["//a:lint"]);
        let second = full(original, "ce\n", &["//b:format"]);
        let forward = reconcile_fixture(
            "reconcile-order-forward",
            "ab\n",
            vec![first.clone(), second.clone()],
            vec![],
        );
        let backward = reconcile_fixture(
            "reconcile-order-backward",
            "ab\n",
            vec![second, first],
            vec![],
        );
        assert_eq!(forward.0.len(), backward.0.len());
        assert_eq!(forward.2, backward.2);
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
