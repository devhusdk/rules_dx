use std::collections::BTreeSet;
use std::path::Path;

use dx_test_scratch::{TempDir, scratch, write};

use super::*;

const DX_TOML: &str = "schema_version = 1\n";
const RUFF_BASE: &str = ".dx/config/ruff/base.toml";

/// Runs a sync with a detector that finds `families` and counts its calls.
fn sync_with(
    root: &Path,
    mode: Mode,
    families: &[&str],
    count: &mut usize,
) -> Result<SyncReport, SyncError> {
    let mut detect = |_: &Path| {
        *count += 1;
        families.iter().map(|family| (*family).to_owned()).collect()
    };
    sync(root, mode, &mut detect)
}

fn try_run(root: &Path, mode: Mode, count: &mut usize) -> Result<SyncReport, SyncError> {
    sync_with(root, mode, &["python"], count)
}

fn run(root: &Path, mode: Mode, count: &mut usize) -> SyncReport {
    try_run(root, mode, count).expect("sync")
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
    assert!(!dir.path().join(".dx/config/ruff").exists());
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

#[test]
fn a_referenced_dx_file_dx_does_not_render_is_kept_with_a_warning() {
    let dir = synced();
    let root = dir.path();
    write(
        root,
        ".dx/config/eslint/extra.mjs",
        "import \"./frameworks.mjs\";\n",
    );
    write(root, ".dx/config/eslint/frameworks.mjs", "x");
    write(root, ".dx/config/eslint/standard.mjs", "x");
    write(
        root,
        "eslint.config.mjs",
        "export { default } from \"./.dx/config/eslint/extra.mjs\";\n",
    );
    write(
        root,
        "tsconfig.json",
        "{ \"extends\": \"./.dx/config/tsconfig/base.json\" }\n",
    );
    let report = run(root, Mode::Fast, &mut 0);
    assert_eq!(
        report.warnings,
        [
            "dx: warning: eslint.config.mjs references .dx/config/eslint/extra.mjs, which dx does not render; kept it",
            "dx: warning: tsconfig.json references .dx/config/tsconfig/base.json, which does not exist",
        ]
    );
    assert_eq!(report.deleted, Vec::<String>::new());
    for rel in ["extra.mjs", "frameworks.mjs", "standard.mjs"] {
        assert!(root.join(".dx/config/eslint").join(rel).is_file(), "{rel}");
    }
    assert_eq!(run(root, Mode::Fast, &mut 0), SyncReport::default());

    write(root, "eslint.config.mjs", "export default [];\n");
    let report = run(root, Mode::Fast, &mut 0);
    assert_eq!(report.deleted.len(), 3, "{report:?}");
}

#[test]
fn reference_files_cover_the_targets_and_the_build_files_above_them() {
    let dir = scratch("references");
    let root = dir.path();
    for rel in [
        "configs/BUILD.bazel",
        "configs/base.toml",
        "configs/other.toml",
        "styles/a.yml",
        "styles/old.bak",
        "styles/sub/b.yml",
        "unrelated/c.txt",
    ] {
        write(root, rel, "x");
    }
    write(root, ".gitignore", "*.bak\n");
    let targets = BTreeSet::from(["configs/base.toml".to_owned(), "styles".to_owned()]);
    let expected: BTreeSet<String> = [
        "configs/BUILD.bazel",
        "configs/base.toml",
        "styles/a.yml",
        "styles/sub/b.yml",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(reference_files(root, &targets), expected);
}

#[test]
fn a_new_file_under_the_vale_styles_rerenders() {
    let markdown = |root: &Path, count: &mut usize| {
        sync_with(root, Mode::Fast, &["markdown"], count).expect("sync")
    };
    let dir = workspace();
    write(
        dir.path(),
        "dx.toml",
        "schema_version = 1\n[families.markdown]\nlint = [\"vale\"]\n",
    );
    write(dir.path(), ".vale.ini", "StylesPath = styles\n");
    write(dir.path(), "styles/a.yml", "x");
    markdown(dir.path(), &mut 0);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("\"//:styles/a.yml\""));
    let mut count = 0;
    markdown(dir.path(), &mut count);
    assert_eq!(
        count, 0,
        "the root BUILD this sync wrote must not cause a miss"
    );

    write(dir.path(), "styles/b.yml", "x");
    markdown(dir.path(), &mut count);
    assert_eq!(count, 1);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("\"//:styles/b.yml\""));
}

#[test]
fn a_build_file_above_a_ruff_extend_target_rerenders() {
    let dir = synced();
    write(dir.path(), "ruff.toml", "extend = \"configs/base.toml\"\n");
    write(dir.path(), "configs/base.toml", "line-length = 100\n");
    run(dir.path(), Mode::Fast, &mut 0);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("\"//:configs/base.toml\""));

    write(dir.path(), "configs/BUILD.bazel", "");
    let mut count = 0;
    run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    let build = read(dir.path(), ".dx/BUILD.bazel");
    assert!(build.contains("\"//configs:base.toml\""), "{build}");
    assert!(!read(dir.path(), "BUILD.bazel").contains("configs/base.toml"));
}

#[test]
fn removing_a_user_export_rerenders() {
    let dir = synced();
    write(dir.path(), "ruff.toml", "line-length = 100\n");
    write(
        dir.path(),
        "BUILD.bazel",
        "exports_files([\"ruff.toml\"])\n",
    );
    let report = run(dir.path(), Mode::Fast, &mut 0);
    assert!(
        !report.written.iter().any(|path| path == "BUILD.bazel"),
        "{report:?}"
    );

    write(dir.path(), "BUILD.bazel", "");
    let mut count = 0;
    run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    assert!(read(dir.path(), "BUILD.bazel").contains("ruff.toml"));
}

#[test]
fn a_dx_toml_edit_during_a_sync_is_not_marked_synced() {
    let dir = workspace();
    let mut detect = |root: &Path| {
        write(root, "dx.toml", "schema_version = 1\ntier = \"strict\"\n");
        BTreeSet::from(["python".to_owned()])
    };
    sync(dir.path(), Mode::Fast, &mut detect).expect("sync");
    assert!(!read(dir.path(), ".dx/BUILD.bazel").contains("profile = \"strict\""));

    let mut count = 0;
    run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 1);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("profile = \"strict\""));
}

#[test]
fn a_failing_sync_is_not_retried_until_an_input_changes() {
    let dir = synced();
    write(dir.path(), "ruff.toml", "extend = \"configs/base.toml\"\n");
    let mut count = 0;
    let first = try_run(dir.path(), Mode::Fast, &mut count).expect_err("missing target");
    assert!(matches!(first, SyncError::Generate(_)), "{first:?}");
    assert_eq!(count, 1);

    let second = try_run(dir.path(), Mode::Fast, &mut count).expect_err("still missing");
    assert!(matches!(second, SyncError::Unchanged(_)), "{second:?}");
    assert_eq!(second.to_string(), first.to_string());
    assert_eq!(count, 1);

    write(dir.path(), "configs/base.toml", "line-length = 100\n");
    run(dir.path(), Mode::Fast, &mut count);
    assert_eq!(count, 2);
    assert!(read(dir.path(), ".dx/BUILD.bazel").contains("\"//:configs/base.toml\""));
}

#[test]
fn concurrent_full_syncs_both_succeed() {
    let dir = workspace();
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..2)
            .map(|_| scope.spawn(|| try_run(dir.path(), Mode::Full, &mut 0)))
            .collect();
        for handle in runs {
            handle.join().expect("thread").expect("sync");
        }
    });
    assert!(dir.path().join(RUFF_BASE).exists());
}

#[test]
fn plan_writes_nothing() {
    let dir = workspace();
    let mut detect = |_: &Path| BTreeSet::from(["python".to_owned()]);
    let files = plan(dir.path(), &mut detect).expect("plan");
    assert!(files.contains_key(RUFF_BASE), "{:?}", files.keys());
    assert!(!dir.path().join(".dx").exists());
}
