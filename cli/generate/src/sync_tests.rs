use std::collections::BTreeSet;
use std::path::Path;

use dx_test_scratch::{TempDir, scratch, write};

use super::*;

const DX_TOML: &str = "schema_version = 1\n";
const RUFF_BASE: &str = ".dx/config/ruff/base.toml";

/// Runs a sync with a detector that finds python and counts its calls.
fn run(root: &Path, mode: Mode, count: &mut usize) -> SyncReport {
    let mut detect = |_: &Path| {
        *count += 1;
        BTreeSet::from(["python".to_owned()])
    };
    sync(root, mode, &mut detect).expect("sync")
}

fn workspace() -> TempDir {
    let dir = scratch("sync");
    write(dir.path(), "dx.toml", DX_TOML);
    write(dir.path(), "MODULE.bazel", "module(name = \"ws\")\n");
    dir
}

fn synced() -> TempDir {
    let dir = workspace();
    let report = run(dir.path(), Mode::Fast, &mut 0);
    assert!(report.detected);
    assert!(
        report.written.iter().any(|path| path == RUFF_BASE),
        "{report:?}"
    );
    dir
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).expect("read")
}

#[test]
fn a_second_fast_run_does_nothing() {
    let dir = synced();
    let fingerprint = dx_path::state::fingerprint_path(dir.path());
    let before = std::fs::metadata(&fingerprint)
        .and_then(|m| m.modified())
        .expect("mtime");
    let mut count = 0;
    let report = run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 0);
    assert_eq!(report, SyncReport::default());
    let after = std::fs::metadata(&fingerprint)
        .and_then(|m| m.modified())
        .expect("mtime");
    assert_eq!(before, after);
}

#[test]
fn full_mode_always_detects() {
    let dir = synced();
    let mut count = 0;
    let report = run(dir.path(), Mode::Full, &mut count);
    assert_eq!(count, 1);
    assert!(report.detected);
    assert!(
        report.written.is_empty() && report.deleted.is_empty(),
        "{report:?}"
    );
}

#[test]
fn removing_a_tool_deletes_its_base() {
    let dir = synced();
    write(
        dir.path(),
        "dx.toml",
        "schema_version = 1\n[families.python]\ndisabled = true\n",
    );
    let mut count = 0;
    let report = run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    assert!(
        report.deleted.iter().any(|path| path == RUFF_BASE),
        "{report:?}"
    );
    assert!(!dir.path().join(RUFF_BASE).exists());
    assert!(!dir.path().join(".dx/config").exists());
}

#[test]
fn editing_dx_toml_rerenders() {
    let dir = synced();
    write(
        dir.path(),
        "dx.toml",
        "schema_version = 1\ntier = \"strict\"\n",
    );
    let mut count = 0;
    let report = run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    assert!(
        report.written.iter().any(|path| path == ".dx/BUILD.bazel"),
        "{report:?}"
    );
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("profile = \"strict\""));
}

#[test]
fn a_root_ruff_toml_rerenders() {
    let dir = synced();
    assert!(!read(dir.path(), ".dx/BUILD.bazel").contains("ruff_config"));
    write(dir.path(), "ruff.toml", "line-length = 100\n");
    let mut count = 0;
    run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("ruff_config"));
    assert!(read(dir.path(), "BUILD.bazel").contains("ruff.toml"));
}

#[test]
fn a_deleted_output_is_restored() {
    let dir = synced();
    let base = read(dir.path(), RUFF_BASE);
    std::fs::remove_file(dir.path().join(RUFF_BASE)).expect("remove");
    let report = run(dir.path(), Mode::Fast, &mut 0);
    assert_eq!(report.written, vec![RUFF_BASE.to_owned()]);
    assert_eq!(read(dir.path(), RUFF_BASE), base);
}

#[test]
fn local_and_pins_survive_and_strays_go() {
    let dir = synced();
    write(dir.path(), ".dx/pins", "dx = \"1.0.0\"\n");
    write(dir.path(), ".dx/local/setups/keep", "x");
    write(dir.path(), ".dx/stray/old.txt", "x");
    write(dir.path(), ".dx/config/local", "x");
    write(dir.path(), "notes.txt", "x");
    let report = run(dir.path(), Mode::Fast, &mut 0);
    assert_eq!(
        report.deleted,
        vec![
            ".dx/config/local".to_owned(),
            ".dx/stray/old.txt".to_owned()
        ]
    );
    assert!(dir.path().join(".dx/pins").exists());
    assert!(dir.path().join(".dx/local/setups/keep").exists());
    assert!(!dir.path().join(".dx/stray").exists());
    assert!(dir.path().join("notes.txt").exists());
}
