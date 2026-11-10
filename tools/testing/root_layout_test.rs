use std::collections::BTreeSet;

/// Root directories every layout keeps.
const SHARED: &[&str] = &[
    "config",
    "cli",
    "core",
    "docs",
    "examples",
    "gazelle",
    "patches",
    "third_party",
    "tools",
];

/// Root directories that have not moved into the layout yet; later changes shrink this list.
const PENDING: &[&str] = &[
    "appimage",
    "bcr_test_module",
    "ci",
    "containers",
    "deploy",
    "dx",
    "env",
    "families",
    "quality",
];

/// Root directories that are build or tool output, not sources.
fn is_artifact(name: &str) -> bool {
    name.starts_with('.') || name.starts_with("bazel-") || name == "node_modules"
}

fn root_directories() -> Vec<String> {
    let root = dx_testing::rules_dx_source();
    let entries = std::fs::read_dir(&root).unwrap_or_else(|error| panic!("{root:?}: {error}"));
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("{root:?}: {error}"));
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() && !is_artifact(&name) {
            names.push(name);
        }
    }
    names.sort();
    names
}

#[test]
fn every_root_directory_is_a_language_or_allowlisted() {
    let root = dx_testing::rules_dx_source();
    let allowed: BTreeSet<&str> = SHARED.iter().chain(PENDING).copied().collect();
    let stray: Vec<String> = root_directories()
        .into_iter()
        .filter(|name| !root.join(name).join("lang.json").is_file())
        .filter(|name| !allowed.contains(name.as_str()))
        .collect();
    assert!(
        stray.is_empty(),
        "root directories that are neither a language with lang.json nor allowlisted: {stray:?}"
    );
}

#[test]
fn pending_directories_still_exist() {
    let present = root_directories();
    let gone: Vec<&str> = PENDING
        .iter()
        .copied()
        .filter(|name| !present.iter().any(|dir| dir == name))
        .collect();
    assert!(gone.is_empty(), "drop these from PENDING: {gone:?}");
}
