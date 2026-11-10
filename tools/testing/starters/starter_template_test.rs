//! Checks the files each starter fixture adds to `dx init` output are all generated and declare the expected bazel_dep names.

use starter_scaffold as scaffold;

#[test]
fn starters_generate_their_files_and_deps() {
    let starters = std::env::var("DX_STARTERS").unwrap_or_else(|_| panic!("DX_STARTERS must be set"));
    let scratch = dx_test_scratch::scratch("starters");
    let mut checked = 0;
    for starter in starters.split(';').filter(|entry| !entry.is_empty()) {
        let (fixture, deps) = starter.split_once(':').unwrap_or((starter, ""));
        let key = format!("DX_FIXTURE_FILES_{}", fixture.replace('-', "_"));
        let files = std::env::var(&key).unwrap_or_else(|_| panic!("{key} must be set"));
        let dir = scratch.path().join(fixture);
        std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir {dir:?}: {error}"));
        check_fixture(&dir, fixture, deps, &files);
        checked += 1;
    }
    assert!(checked > 0, "DX_STARTERS lists no fixtures");
}

fn check_fixture(scratch: &std::path::Path, fixture: &str, deps: &str, files: &str) {
    let root = scaffold::scaffold_fixture(scratch, fixture, "demo-app", files);

    let prefix = format!("{}/{fixture}/", scaffold::FIXTURES_DIR);
    let common = format!("{}/common/", scaffold::FIXTURES_DIR);
    let mut paths: Vec<&str> = files
        .split_whitespace()
        .filter_map(|file| {
            let rest = file
                .strip_prefix(&prefix)
                .or_else(|| file.strip_prefix(&common)?.split_once('/').map(|split| split.1))?;
            rest.strip_suffix(".fixture")
        })
        .collect();
    paths.sort_unstable();
    for path in paths {
        assert!(root.join(path).is_file(), "{fixture}: {path} was not generated");
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
    for path in std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.path()) {
        if path.extension().is_some_and(|ext| ext == "bazel" || ext == "bzl") {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                !text.contains("/rules:defs.bzl"),
                "{fixture}: {path:?} loads a /rules:defs.bzl label"
            );
            assert!(
                !text.contains("@rules_dx_"),
                "{fixture}: {path:?} loads a @rules_dx_ repository"
            );
        }
    }
}
