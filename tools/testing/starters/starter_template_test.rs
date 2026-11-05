//! Checks the files a starter fixture adds to `dx init` output against a golden listing.

use starter_scaffold as scaffold;

#[test]
fn starter_matches_its_golden() {
    let fixture = std::env::var("DX_FIXTURE").unwrap_or_else(|_| panic!("DX_FIXTURE must be set"));
    let golden = std::env::var("DX_GOLDEN").unwrap_or_else(|_| panic!("DX_GOLDEN must be set"));
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
    let mut lines = Vec::new();
    for path in paths {
        assert!(root.join(path).is_file(), "{path} was not generated");
        lines.push(path.to_owned());
    }
    let module = std::fs::read_to_string(root.join("MODULE.bazel")).unwrap_or_default();
    for line in module.lines().filter(|line| line.starts_with("bazel_dep(")) {
        lines.push(format!(
            "bazel_dep {}",
            line.split('"').nth(1).unwrap_or_default()
        ));
    }
    let got = format!("{}\n", lines.join("\n"));
    let _ = std::fs::remove_dir_all(&scratch);

    let want = std::fs::read_to_string(dx_testing::resolve_runfiles(&golden))
        .unwrap_or_else(|error| panic!("read {golden}: {error}"));
    assert_eq!(got, want, "{fixture} differs from {golden}");
}
