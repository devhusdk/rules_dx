use dx_apply::FileSystem;
use dx_exec_core::common::{
    FileChange, REASON_INCOMPLETE_COLLECTION, REASON_INVALID_EDITS, REASON_OUTSIDE_SCOPE,
    REASON_OUTSIDE_WORKSPACE, REASON_STALE_SOURCE, REASON_UNREADABLE_SOURCE, REASON_WRITE_FAILED,
    SourceRead, apply_to_bytes, read_verified,
};
use dx_output::{DiagnosticEvent, Threshold, meets_threshold};
use dx_plan::resolve::path_in_filter;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) struct ApplyOutcome {
    pub(crate) sources: BTreeMap<String, SourceRead>,
    pub(crate) applied: BTreeMap<String, bool>,
    pub(crate) not_applied: Vec<(String, &'static str)>,
}

/// Resolves links so a write lands on the real file, never replacing the link itself.
fn resolve_source(workspace: &Path, path: &str) -> Result<PathBuf, &'static str> {
    match dx_path::workspace::resolve_target_in_workspace(workspace, path) {
        Ok(target) => Ok(target),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
            Err(REASON_OUTSIDE_WORKSPACE)
        }
        Err(_) => Err(REASON_UNREADABLE_SOURCE),
    }
}

/// Writes each verified candidate through `fs`; a failed write is reported
/// as `write_failed` and does not roll back files already written. With
/// `writable`, a change to any other path is reported as `outside_scope`.
/// A path that links out of the workspace is never read and is reported as
/// `outside_workspace`.
pub(crate) fn apply_collected_changes(
    fs: &dyn FileSystem,
    workspace: &Path,
    apply: bool,
    complete: bool,
    writable: Option<&[String]>,
    changes: &[FileChange],
) -> ApplyOutcome {
    let mut targets: BTreeMap<String, Result<PathBuf, &'static str>> = BTreeMap::new();
    let mut sources: BTreeMap<String, SourceRead> = BTreeMap::new();
    for change in changes {
        let target = targets
            .entry(change.path.clone())
            .or_insert_with(|| resolve_source(workspace, &change.path));
        sources
            .entry(change.path.clone())
            .or_insert_with(|| match target {
                Ok(file) => read_verified(file, &change.original_digest),
                Err(_) => SourceRead::Unreadable,
            });
    }

    let mut applied: BTreeMap<String, bool> = BTreeMap::new();
    let mut not_applied: Vec<(String, &'static str)> = Vec::new();
    if !apply {
    } else if !complete {
        for change in changes {
            applied.insert(change.path.clone(), false);
            not_applied.push((change.path.clone(), REASON_INCOMPLETE_COLLECTION));
        }
    } else {
        for change in changes {
            if writable.is_some_and(|paths| !path_in_filter(&change.path, paths)) {
                applied.insert(change.path.clone(), false);
                not_applied.push((change.path.clone(), REASON_OUTSIDE_SCOPE));
                continue;
            }
            let reason = match (targets.get(&change.path), sources.get(&change.path)) {
                (Some(Err(reason)), _) => Some(*reason),
                (Some(Ok(file)), Some(SourceRead::Bytes(original))) => {
                    match apply_to_bytes(original, &change.edits) {
                        Some(candidate) => {
                            if !matches!(
                                read_verified(file, &change.original_digest),
                                SourceRead::Bytes(_)
                            ) {
                                applied.insert(change.path.clone(), false);
                                not_applied.push((change.path.clone(), REASON_STALE_SOURCE));
                                continue;
                            }
                            match fs.write_atomic(file, &candidate) {
                                Ok(()) => {
                                    applied.insert(change.path.clone(), true);
                                    None
                                }
                                Err(_) => Some(REASON_WRITE_FAILED),
                            }
                        }
                        None => Some(REASON_INVALID_EDITS),
                    }
                }
                (_, Some(SourceRead::Unreadable)) => Some(REASON_UNREADABLE_SOURCE),
                (_, Some(SourceRead::Stale) | None) | (None, _) => Some(REASON_STALE_SOURCE),
            };
            if let Some(reason) = reason {
                applied.insert(change.path.clone(), false);
                not_applied.push((change.path.clone(), reason));
            }
        }
    }
    ApplyOutcome {
        sources,
        applied,
        not_applied,
    }
}

fn reappears(initial: &DiagnosticEvent, terminal: &[DiagnosticEvent], used: &mut [bool]) -> bool {
    let found = terminal.iter().enumerate().position(|(index, later)| {
        used.get(index) == Some(&false)
            && later.tool == initial.tool
            && later.rule == initial.rule
            && later.path == initial.path
    });
    if let Some(slot) = found.and_then(|index| used.get_mut(index)) {
        *slot = true;
    }
    found.is_some()
}

/// Marks unwritten paths whose terminal bytes differ from the original as not on disk.
pub(crate) fn terminal_on_disk(
    applied: &BTreeMap<String, bool>,
    original_digests: &BTreeMap<String, [u8; 32]>,
    terminal_digests: &BTreeMap<String, [u8; 32]>,
) -> BTreeMap<String, bool> {
    let mut on_disk = applied.clone();
    for path in original_digests.keys().chain(terminal_digests.keys()) {
        if !on_disk.contains_key(path) && original_digests.get(path) != terminal_digests.get(path) {
            on_disk.insert(path.clone(), false);
        }
    }
    on_disk
}

pub(crate) fn project_status(
    apply: bool,
    initial: &[DiagnosticEvent],
    terminal: &[DiagnosticEvent],
    on_disk: &BTreeMap<String, bool>,
    fail_on: Threshold,
    has_changes: bool,
) -> (Vec<DiagnosticEvent>, bool) {
    let mut status: Vec<DiagnosticEvent> = Vec::new();
    if apply {
        status.extend(
            terminal
                .iter()
                .filter(|diagnostic| {
                    diagnostic
                        .path
                        .as_ref()
                        .is_none_or(|path| on_disk.get(path).copied().unwrap_or(true))
                })
                .cloned(),
        );
        let mut used = vec![false; terminal.len()];
        for diagnostic in initial {
            let written = match diagnostic.path.as_ref() {
                Some(path) => on_disk.get(path).copied(),
                None => None,
            };
            let covered = match written {
                Some(true) => diagnostic.fixable || reappears(diagnostic, terminal, &mut used),
                Some(false) => false,
                None => reappears(diagnostic, terminal, &mut used),
            };
            if covered {
                continue;
            }
            let mut unmatched = diagnostic.clone();
            if written == Some(true) {
                unmatched.range = None;
                for related in &mut unmatched.related {
                    let related_written = match related.path.as_ref() {
                        Some(path) => on_disk.get(path).copied() == Some(true),
                        None => true,
                    };
                    if related_written {
                        related.range = None;
                    }
                }
            }
            status.push(unmatched);
        }
        dx_output::sort_diagnostics(&mut status);
    } else {
        status.extend(initial.iter().cloned());
    }
    let failing = status
        .iter()
        .any(|diagnostic| meets_threshold(diagnostic.severity, fail_on));
    let mut failed = failing;
    if !apply && has_changes {
        failed = true;
    }
    (status, failed)
}

#[cfg(test)]
mod status_tests {
    use super::{project_status, terminal_on_disk};
    use dx_output::{DiagnosticEvent, Severity, Snapshot, Threshold};
    use std::collections::BTreeMap;

    fn event(snapshot: Snapshot, fixable: bool) -> DiagnosticEvent {
        event_with(snapshot, fixable, "m")
    }

    fn event_with(snapshot: Snapshot, fixable: bool, message: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            path: Some("a.py".to_owned()),
            range: Some((0, 1)),
            snapshot,
            fixable,
            ..DiagnosticEvent::new(Severity::Warning, "lint", message)
        }
    }

    #[test]
    fn failed_write_keeps_original_findings_and_drops_unwritten_terminal_ones() {
        let applied = BTreeMap::from([("a.py".to_owned(), false)]);
        let (status, failed) = project_status(
            true,
            &[event(Snapshot::Initial, true)],
            &[event(Snapshot::Terminal, false)],
            &applied,
            Threshold::Warning,
            true,
        );
        assert!(failed);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Initial);
    }

    #[test]
    fn written_file_reports_terminal_findings() {
        let applied = BTreeMap::from([("a.py".to_owned(), true)]);
        let (status, _) = project_status(
            true,
            &[event(Snapshot::Initial, true)],
            &[event(Snapshot::Terminal, false)],
            &applied,
            Threshold::Warning,
            true,
        );
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn written_file_does_not_repeat_unfixable_original_findings() {
        let applied = BTreeMap::from([("a.py".to_owned(), true)]);
        let (status, failed) = project_status(
            true,
            &[event(Snapshot::Initial, false)],
            &[event(Snapshot::Terminal, false)],
            &applied,
            Threshold::Warning,
            true,
        );
        assert!(failed);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn written_file_keeps_unmatched_unfixable_finding_on_its_original_snapshot() {
        let applied = BTreeMap::from([("a.py".to_owned(), true)]);
        let (status, failed) = project_status(
            true,
            &[event(Snapshot::Initial, false)],
            &[],
            &applied,
            Threshold::Warning,
            true,
        );
        assert!(failed);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Initial);
        assert_eq!(status[0].range, None);
    }

    #[test]
    fn unfixable_finding_with_changed_message_is_reported_once() {
        let applied = BTreeMap::from([("a.py".to_owned(), true)]);
        let (status, _) = project_status(
            true,
            &[event_with(Snapshot::Initial, false, "3 issues at line 9")],
            &[event_with(Snapshot::Terminal, false, "3 issues at line 7")],
            &applied,
            Threshold::Warning,
            true,
        );
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn unchanged_file_reports_each_finding_once() {
        let (status, failed) = project_status(
            true,
            &[event(Snapshot::Initial, false)],
            &[event(Snapshot::Terminal, false)],
            &BTreeMap::new(),
            Threshold::Warning,
            false,
        );
        assert!(failed);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    fn unwritten_status(
        initial: &[DiagnosticEvent],
        terminal: &[DiagnosticEvent],
        original: Option<u8>,
        current: Option<u8>,
    ) -> Vec<DiagnosticEvent> {
        let digests = |byte: Option<u8>| {
            byte.map_or_else(BTreeMap::new, |byte| {
                BTreeMap::from([("a.py".to_owned(), [byte; 32])])
            })
        };
        let on_disk = terminal_on_disk(&BTreeMap::new(), &digests(original), &digests(current));
        project_status(true, initial, terminal, &on_disk, Threshold::Warning, false).0
    }

    #[test]
    fn unwritten_file_with_differing_digests_reports_only_the_original_finding() {
        let status = unwritten_status(
            &[event(Snapshot::Initial, false)],
            &[event(Snapshot::Terminal, false)],
            Some(1),
            Some(2),
        );
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Initial);
    }

    #[test]
    fn unwritten_file_with_equal_digests_reports_the_terminal_finding_once() {
        let status = unwritten_status(
            &[event(Snapshot::Initial, false)],
            &[event(Snapshot::Terminal, false)],
            Some(1),
            Some(1),
        );
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn pathless_finding_is_reported_once() {
        let mut initial = event(Snapshot::Initial, false);
        initial.path = None;
        initial.range = None;
        let mut terminal = event(Snapshot::Terminal, false);
        terminal.path = None;
        terminal.range = None;
        let status = unwritten_status(&[initial], &[terminal], Some(1), Some(2));
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn file_with_only_an_original_digest_keeps_the_original_finding() {
        let status = unwritten_status(
            &[event(Snapshot::Initial, false)],
            &[event(Snapshot::Terminal, false)],
            Some(1),
            None,
        );
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].snapshot, Snapshot::Initial);
    }

    #[test]
    fn terminal_on_disk_keeps_written_paths_and_skips_unknown_ones() {
        let applied = BTreeMap::from([("a.py".to_owned(), true)]);
        let original = BTreeMap::from([
            ("a.py".to_owned(), [1; 32]),
            ("b.py".to_owned(), [1; 32]),
            ("c.py".to_owned(), [1; 32]),
        ]);
        let terminal = BTreeMap::from([
            ("a.py".to_owned(), [2; 32]),
            ("b.py".to_owned(), [1; 32]),
            ("c.py".to_owned(), [3; 32]),
            ("d.py".to_owned(), [4; 32]),
        ]);
        assert_eq!(
            terminal_on_disk(&applied, &original, &terminal),
            BTreeMap::from([
                ("a.py".to_owned(), true),
                ("c.py".to_owned(), false),
                ("d.py".to_owned(), false),
            ])
        );
    }
}

#[cfg(test)]
#[path = "quality_apply_tests.rs"]
mod tests;
