use super::common::{
    apply_to_bytes, read_verified, FileChange, SourceRead, REASON_INCOMPLETE_COLLECTION,
    REASON_INVALID_EDITS, REASON_STALE_SOURCE, REASON_UNREADABLE_SOURCE,
};
use dx_apply::{FileSystem, RealFileSystem};
use dx_output::{meets_threshold, DiagnosticEvent, Threshold};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct ApplyOutcome {
    pub(crate) sources: BTreeMap<String, SourceRead>,
    pub(crate) applied: BTreeMap<String, bool>,
    pub(crate) not_applied: Vec<(String, &'static str)>,
}

pub(crate) fn apply_collected_changes(
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

    let fs = RealFileSystem;
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
                            match fs.write_atomic(&workspace.join(&change.path), &candidate) {
                                Ok(()) => {
                                    applied.insert(change.path.clone(), true);
                                    None
                                }
                                Err(_) => Some(REASON_UNREADABLE_SOURCE),
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

pub(crate) fn project_status(
    apply: bool,
    initial: &[DiagnosticEvent],
    terminal: &[DiagnosticEvent],
    applied: &BTreeMap<String, bool>,
    fail_on: Threshold,
    has_changes: bool,
) -> (Vec<DiagnosticEvent>, bool) {
    let mut status: Vec<DiagnosticEvent> = Vec::new();
    if !apply {
        status.extend(initial.iter().cloned());
    } else {
        status.extend(terminal.iter().cloned());
        for diagnostic in initial {
            let is_applied = diagnostic
                .path
                .as_ref()
                .is_some_and(|path| applied.get(path).copied().unwrap_or(false));
            if is_applied && diagnostic.fixable {
                continue;
            }
            status.push(diagnostic.clone());
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
        REASON_UNREADABLE_SOURCE,
    };
    use super::super::test_support::temp_dir;
    use super::apply_collected_changes;
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
        let outcome = apply_collected_changes(workspace.path(), false, true, &[change]);
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
        let outcome = apply_collected_changes(workspace.path(), true, false, &[change]);
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
        let outcome = apply_collected_changes(workspace.path(), true, true, &[valid, stale]);
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
    #[cfg(unix)]
    fn failed_write_leaves_source_untouched() {
        use std::os::unix::fs::PermissionsExt as _;
        let workspace = write_workspace("apply-write-fails", &[("src/a.rs", b"BAD\n")]);
        let dir = workspace.path().join("src");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555))
            .expect("make dir read-only");
        let change = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
        let outcome = apply_collected_changes(workspace.path(), true, true, &[change]);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore dir");
        assert_eq!(
            std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
            b"BAD\n"
        );
        assert_eq!(outcome.applied.get("src/a.rs"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("src/a.rs".to_owned(), REASON_UNREADABLE_SOURCE)]
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
        let outcome = apply_collected_changes(workspace.path(), true, true, &[change]);
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
