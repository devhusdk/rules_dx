use super::common::{
    apply_to_bytes, read_verified, FileChange, SourceRead, REASON_INCOMPLETE_COLLECTION,
    REASON_INVALID_EDITS, REASON_STALE_SOURCE, REASON_UNREADABLE_SOURCE, REASON_WRITE_FAILED,
};
use dx_apply::FileSystem;
use dx_output::{meets_threshold, DiagnosticEvent, Threshold};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct ApplyOutcome {
    pub(crate) sources: BTreeMap<String, SourceRead>,
    pub(crate) applied: BTreeMap<String, bool>,
    pub(crate) not_applied: Vec<(String, &'static str)>,
}

/// Writes each verified candidate through `fs`; a failed write is reported
/// as `write_failed` and does not roll back files already written.
pub(crate) fn apply_collected_changes(
    fs: &dyn FileSystem,
    workspace: &Path,
    apply: bool,
    complete: bool,
    changes: &[FileChange],
) -> ApplyOutcome {
    let mut sources: BTreeMap<String, SourceRead> = BTreeMap::new();
    for change in changes {
        sources
            .entry(change.path.clone())
            .or_insert_with(|| read_verified(workspace, &change.path, &change.original_digest));
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
            let reason = match sources.get(&change.path) {
                Some(SourceRead::Bytes(original)) => {
                    match apply_to_bytes(original, &change.edits) {
                        Some(candidate) => {
                            if !matches!(
                                read_verified(workspace, &change.path, &change.original_digest),
                                SourceRead::Bytes(_)
                            ) {
                                applied.insert(change.path.clone(), false);
                                not_applied.push((change.path.clone(), REASON_STALE_SOURCE));
                                continue;
                            }
                            match fs.write_atomic(&workspace.join(&change.path), &candidate) {
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
                Some(SourceRead::Unreadable) => Some(REASON_UNREADABLE_SOURCE),
                Some(SourceRead::Stale) | None => Some(REASON_STALE_SOURCE),
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
        !used[index]
            && later.tool == initial.tool
            && later.rule == initial.rule
            && later.path == initial.path
    });
    if let Some(index) = found {
        used[index] = true;
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
    if !apply {
        status.extend(initial.iter().cloned());
    } else {
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
mod tests {
    use super::super::common::{
        FileChange, REASON_INCOMPLETE_COLLECTION, REASON_INVALID_EDITS, REASON_STALE_SOURCE,
        REASON_WRITE_FAILED,
    };
    use super::super::test_support::{temp_dir, FailingWrites};
    use super::apply_collected_changes;
    use dx_apply::RealFileSystem;
    use dx_digest::blake3 as digest;

    fn write_workspace(name: &str, files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let workspace = temp_dir(name);
        for (path, bytes) in files {
            let full = workspace.path().join(path);
            std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
            std::fs::write(&full, bytes).expect("write source");
        }
        workspace
    }

    fn full_replace(path: &str, original: &[u8], terminal: &str) -> FileChange {
        FileChange {
            path: path.to_owned(),
            original_digest: digest(original),
            edits: vec![(0, original.len() as u64, terminal.to_owned())],
        }
    }

    #[test]
    fn check_mode_writes_nothing() {
        let workspace = write_workspace("apply-check", &[("src/a.rs", b"BAD\n")]);
        let change = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let outcome =
            apply_collected_changes(&RealFileSystem, workspace.path(), false, true, &[change]);
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert!(outcome.applied.is_empty());
        assert!(outcome.not_applied.is_empty());
    }

    #[test]
    fn incomplete_collection_writes_nothing() {
        let workspace = write_workspace("apply-incomplete", &[("src/a.rs", b"BAD\n")]);
        let change = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let outcome =
            apply_collected_changes(&RealFileSystem, workspace.path(), true, false, &[change]);
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("src/a.rs".to_owned(), REASON_INCOMPLETE_COLLECTION)]
        );
    }

    #[test]
    fn stale_source_untouched_while_valid_file_applies() {
        let workspace = write_workspace(
            "apply-mixed",
            &[("src/a.rs", b"BAD\n"), ("src/b.rs", b"STALE\n")],
        );
        let valid = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let stale = FileChange {
            path: "src/b.rs".to_owned(),
            original_digest: digest(b"OTHER\n"),
            edits: vec![(0, 6, "NEW\n".to_owned())],
        };
        let outcome = apply_collected_changes(
            &RealFileSystem,
            workspace.path(),
            true,
            true,
            &[valid, stale],
        );
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"GOOD\n"
        );
        assert_eq!(
            std::fs::read(workspace.path().join("src/b.rs")).expect("read back"),
            b"STALE\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&true));
        assert_eq!(outcome.applied.get("src/b.rs"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("src/b.rs".to_owned(), REASON_STALE_SOURCE)]
        );
    }

    #[test]
    fn failed_write_leaves_source_untouched() {
        let workspace = write_workspace("apply-write-fails", &[("src/a.rs", b"BAD\n")]);
        let change = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let fs = FailingWrites::new(&[workspace.path().join("src/a.rs")]);
        let outcome = apply_collected_changes(&fs, workspace.path(), true, true, &[change]);
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("src/a.rs".to_owned(), REASON_WRITE_FAILED)]
        );
    }

    #[test]
    fn failed_write_keeps_other_written_files() {
        let workspace = write_workspace(
            "apply-write-partial",
            &[("src/a.rs", b"BAD\n"), ("src/b.rs", b"ALSO\n")],
        );
        let first = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let second = full_replace("src/b.rs", b"ALSO\n", "FINE\n");
        let fs = FailingWrites::new(&[workspace.path().join("src/a.rs")]);
        let outcome = apply_collected_changes(&fs, workspace.path(), true, true, &[first, second]);
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert_eq!(
            std::fs::read(workspace.path().join("src/b.rs")).expect("read back"),
            b"FINE\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&false));
        assert_eq!(outcome.applied.get("src/b.rs"), Some(&true));
        assert_eq!(
            outcome.not_applied,
            vec![("src/a.rs".to_owned(), REASON_WRITE_FAILED)]
        );
    }

    #[test]
    fn invalid_edits_rejected_without_write() {
        let workspace = write_workspace("apply-invalid", &[("src/a.rs", b"BAD\n")]);
        let change = FileChange {
            path: "src/a.rs".to_owned(),
            original_digest: digest(b"BAD\n"),
            edits: vec![(5, 2, "X".to_owned())],
        };
        let outcome =
            apply_collected_changes(&RealFileSystem, workspace.path(), true, true, &[change]);
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("src/a.rs".to_owned(), REASON_INVALID_EDITS)]
        );
    }
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
