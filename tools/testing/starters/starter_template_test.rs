//! Checks the files a starter fixture adds to `dx init` output are all generated and declare the expected bazel_dep names.

use starter_scaffold as scaffold;

#[test]
fn starter_generates_its_files_and_deps() {
    let fixture = std::env::var("DX_FIXTURE").unwrap_or_else(|_| panic!("DX_FIXTURE must be set"));
    let deps = std::env::var("DX_BAZEL_DEPS").unwrap_or_default();
    let files = std::env::var("DX_FIXTURE_FILES").unwrap_or_default();
    let scratch = std::env::temp_dir().join(format!("starter_template_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap_or_else(|error| panic!("mkdir {scratch:?}: {error}"));
    let root = scaffold::scaffold_fixture(&scratch, &fixture, "demo-app");

    let prefix = format!("{}/{fixture}/", scaffold::FIXTURES_DIR);
    let mut paths: Vec<&str> = files
        .split_whitespace()
        .filter_map(|file| file.strip_prefix(&prefix)?.strip_suffix(".fixture"))
        .filter(|path| *path != "families")
        .collect();
    paths.sort_unstable();
    for path in paths {
        assert!(root.join(path).is_file(), "{path} was not generated");
    }
    let module = std::fs::read_to_string(root.join("MODULE.bazel")).unwrap_or_default();
    let mut got: Vec<&str> = module
        .lines()
        .filter(|line| line.starts_with("bazel_dep("))
        .filter_map(|line| line.split('"').nth(1))
        .collect();
    got.sort_unstable();
    let mut want: Vec<&str> = deps.split_whitespace().collect();
    want.sort_unstable();
    assert_eq!(got, want, "{fixture} bazel_dep names differ");
    let _ = std::fs::remove_dir_all(&scratch);
}
