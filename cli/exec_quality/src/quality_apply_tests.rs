use super::apply_collected_changes;
use dx_apply::RealFileSystem;
use dx_digest::blake3 as digest;
use dx_exec_core::common::{
    FileChange, REASON_INCOMPLETE_COLLECTION, REASON_INVALID_EDITS, REASON_OUTSIDE_SCOPE,
    REASON_OUTSIDE_WORKSPACE, REASON_STALE_SOURCE, REASON_WRITE_FAILED,
};
use dx_exec_quality_testing::{FailingWrites, temp_dir};

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
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        false,
        true,
        None,
        &[change],
    );
    assert_eq!(
        std::fs::read(workspace.path().join("src/a.rs")).expect("read back"),
        b"BAD\n"
    );
    assert!(outcome.applied.is_empty());
    assert!(outcome.not_applied.is_empty(), "{:?}", outcome.not_applied);
}

#[test]
fn incomplete_collection_writes_nothing() {
    let workspace = write_workspace("apply-incomplete", &[("src/a.rs", b"BAD\n")]);
    let change = full_replace("src/a.rs", b"BAD\n", "GOOD\n");
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        true,
        false,
        None,
        &[change],
    );
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
        None,
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
    let outcome = apply_collected_changes(&fs, workspace.path(), true, true, None, &[change]);
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
    let outcome =
        apply_collected_changes(&fs, workspace.path(), true, true, None, &[first, second]);
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

#[cfg(unix)]
#[test]
fn symlinked_source_is_written_through_the_link() {
    let workspace = write_workspace("apply-link", &[("common/shared.py", b"BAD\n")]);
    std::fs::create_dir_all(workspace.path().join("src")).expect("mkdir");
    let link = workspace.path().join("src/shared.py");
    std::os::unix::fs::symlink("../common/shared.py", &link).expect("symlink");
    let change = full_replace("src/shared.py", b"BAD\n", "GOOD\n");
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        true,
        true,
        None,
        &[change],
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("link metadata")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read(workspace.path().join("common/shared.py")).expect("read back"),
        b"GOOD\n"
    );
    assert_eq!(outcome.applied.get("src/shared.py"), Some(&true));
    assert!(outcome.not_applied.is_empty(), "{:?}", outcome.not_applied);
}

#[cfg(unix)]
#[test]
fn symlink_leaving_workspace_is_not_applied() {
    let outside = write_workspace("apply-link-outside", &[("shared.py", b"BAD\n")]);
    let workspace = write_workspace("apply-link-inside", &[]);
    std::fs::create_dir_all(workspace.path().join("src")).expect("mkdir");
    let link = workspace.path().join("src/shared.py");
    std::os::unix::fs::symlink(outside.path().join("shared.py"), &link).expect("symlink");
    let change = full_replace("src/shared.py", b"BAD\n", "GOOD\n");
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        true,
        true,
        None,
        &[change],
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("link metadata")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read(outside.path().join("shared.py")).expect("read back"),
        b"BAD\n"
    );
    assert_eq!(outcome.applied.get("src/shared.py"), Some(&false));
    assert_eq!(
        outcome.not_applied,
        vec![("src/shared.py".to_owned(), REASON_OUTSIDE_WORKSPACE)]
    );
}

#[test]
fn a_change_outside_the_writable_paths_is_refused() {
    let workspace = write_workspace(
        "apply-outside",
        &[("svc/a.rs", b"BAD\n"), ("lib/BUILD.bazel", b"BAD\n")],
    );
    let inside = full_replace("svc/a.rs", b"BAD\n", "GOOD\n");
    let outside = full_replace("lib/BUILD.bazel", b"BAD\n", "GOOD\n");
    let writable = ["svc/a.rs".to_owned(), "svc/BUILD.bazel".to_owned()];
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        true,
        true,
        Some(&writable),
        &[inside, outside],
    );
    assert_eq!(
        std::fs::read(workspace.path().join("svc/a.rs")).expect("read back"),
        b"GOOD\n"
    );
    assert_eq!(
        std::fs::read(workspace.path().join("lib/BUILD.bazel")).expect("read back"),
        b"BAD\n"
    );
    assert_eq!(outcome.applied.get("lib/BUILD.bazel"), Some(&false));
    assert_eq!(
        outcome.not_applied,
        vec![("lib/BUILD.bazel".to_owned(), REASON_OUTSIDE_SCOPE)]
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
    let outcome = apply_collected_changes(
        &RealFileSystem,
        workspace.path(),
        true,
        true,
        None,
        &[change],
    );
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
