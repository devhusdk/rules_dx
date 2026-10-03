use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::plan::OUTPUT_GROUP;
use dx_output::{DiagnosticEvent, Severity, Snapshot};
use quality_result::{decode_validated, proto};

use super::common::{collect_targets, FileChange};

#[derive(Default)]
struct Staged {
    tools: Vec<String>,
    initial: Vec<DiagnosticEvent>,
    terminal: Vec<DiagnosticEvent>,
    changes: Vec<FileChange>,
    digests: Vec<(String, [u8; 32])>,
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
        staged.digests.push((snapshot.path.clone(), digest));
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
        staged.changes.push(map_change(file)?);
    }
    Some(staged)
}

pub(crate) struct Collected {
    pub(crate) tools: Vec<String>,
    pub(crate) initial: Vec<DiagnosticEvent>,
    pub(crate) terminal: Vec<DiagnosticEvent>,
    pub(crate) changes: Vec<FileChange>,
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
            let Some(part) = stage_artifact(&artifact.bytes) else {
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
        changes.extend(staged.changes);
        terminal_digests.extend(staged.digests);
    }
    Ok(Collected {
        tools: tools.into_iter().collect(),
        initial,
        terminal,
        changes,
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
