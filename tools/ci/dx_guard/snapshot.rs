//! The source snapshot the guards scan, and the runfiles their tests name.

use std::collections::BTreeMap;

use crate::starlark::is_starlark;

/// The guard sources, which hold the patterns they detect.
const GUARDS: [&str; 4] = [
    "tools/ci/host_env_test.rs",
    "tools/ci/host_program_test.rs",
    "tools/ci/sandbox_escape_test.rs",
    "tools/ci/verbatim_prefix_test.rs",
];

/// Endings of the scanned files that are not Starlark.
const CODE: [&str; 7] = [".rs", ".sh", ".tpl", ".bazelrc", ".bat", ".cmd", ".ps1"];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 2000;

/// Reads one runfile named by an environment variable.
pub fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

/// The snapshot as a map from path to text, without the guard sources.
pub fn snapshot() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for variable in ["DX_SOURCES_JSON", "DX_FILES_JSON", "DX_CODE_JSON"] {
        let part: BTreeMap<String, String> = serde_json::from_str(&read_input(variable))
            .unwrap_or_else(|error| panic!("{variable}: {error}"));
        files.extend(part);
    }
    assert!(
        files.len() >= MIN_FILES,
        "only {} files in the snapshot",
        files.len()
    );
    files.retain(|path, _| {
        let scanned = is_starlark(path) || CODE.iter().any(|end| path.ends_with(end));
        scanned && !GUARDS.contains(&path.as_str())
    });
    files
}
