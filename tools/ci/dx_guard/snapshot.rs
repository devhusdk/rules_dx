//! The source snapshot the guards scan, and the runfiles their tests name.

use std::collections::BTreeMap;
use std::path::Path;

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

/// Path components that mark a file as test-only or vendored.
const SKIPPED_DIRS: [&str; 5] = ["fixtures", "patches", "testdata", "tests", "third_party"];

/// File name endings that mark a Starlark file as test-only.
const TEST_SUFFIXES: [&str; 2] = ["_test.bzl", "_tests.bzl"];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 2000;

/// Reads one runfile named by an environment variable.
#[expect(clippy::panic, reason = "test helper: a failed setup fails the test")]
pub fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

/// The snapshot as a map from path to text, without the guard sources.
#[expect(clippy::panic, reason = "test helper: a failed setup fails the test")]
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

/// Whether `path` is a test, a fixture, a patch or vendored.
pub fn is_skipped(path: &str) -> bool {
    path.split('/').any(|part| SKIPPED_DIRS.contains(&part))
        || TEST_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
}

/// Every file under `root` as a sorted `(relative path, text)` list.
#[expect(clippy::panic, reason = "test helper: a failed setup fails the test")]
pub fn tree(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("read_dir {}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("read_dir {}: {error}", dir.display()))
                .path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                out.push((rel, text));
            }
        }
    }
    out.sort();
    out
}
