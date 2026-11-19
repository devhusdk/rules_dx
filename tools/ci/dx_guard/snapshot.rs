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
pub fn read_input(variable: &str) -> String {
    let path = dx_testing::rlocation(&dx_testing::env_text(variable));
    assert!(path.is_ok(), "{variable}: {path:?}");
    path.map(|path| dx_testing::read_text(&path))
        .unwrap_or_default()
}

/// The snapshot as a map from path to text, without the guard sources.
pub fn snapshot() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for variable in ["DX_SOURCES_JSON", "DX_FILES_JSON", "DX_CODE_JSON"] {
        let part = serde_json::from_str::<BTreeMap<String, String>>(&read_input(variable));
        assert!(part.is_ok(), "{variable}: {part:?}");
        files.extend(part.unwrap_or_default());
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

/// Whether one directory name of `path` is in `names`.
pub fn under(path: &str, names: &[&str]) -> bool {
    path.rsplit_once('/')
        .is_some_and(|(dir, _)| dir.split('/').any(|part| names.contains(&part)))
}

fn name_match(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => {
            (0..=name.len()).any(|skip| name_match(rest, name.get(skip..).unwrap_or_default()))
        }
        Some((byte, rest)) => name
            .split_first()
            .is_some_and(|(first, tail)| first == byte && name_match(rest, tail)),
    }
}

fn parts_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => {
            (0..=path.len()).any(|skip| parts_match(rest, path.get(skip..).unwrap_or_default()))
        }
        Some((first, rest)) => path.split_first().is_some_and(|(name, tail)| {
            name_match(first.as_bytes(), name.as_bytes()) && parts_match(rest, tail)
        }),
    }
}

/// Whether Bazel glob `pattern` matches `path`.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    parts_match(&pattern, &path)
}

/// Every file under `root` as a sorted `(relative path, text)` list.
///
/// # Errors
///
/// Returns the first directory or file that cannot be read.
pub fn tree(root: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .map_err(std::io::Error::other)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).map_err(|error| {
                    std::io::Error::new(error.kind(), format!("read {}: {error}", path.display()))
                })?;
                out.push((rel, text));
            }
        }
    }
    out.sort();
    Ok(out)
}
