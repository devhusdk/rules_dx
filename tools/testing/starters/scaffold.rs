//! Starts a project with `dx init` and adds a hello-world fixture.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The runfiles directory that holds the hello-world fixtures, one subdirectory per language.
pub const FIXTURES_DIR: &str = "tools/testing/starters/fixtures";

/// Starts `scratch/<name>` the way a user does: `dx init --apply <name>` with the dx named by
/// `DX_BIN`, then the hello-world sources of `fixture`, read from the runfiles that
/// `DX_FIXTURE_FILES` lists (`$(rootpaths //tools/testing/starters/fixtures:<fixture>)`; files of
/// other fixtures in the list are skipped).
///
/// Files under `common/<layer>/` of the fixtures directory are shared layers the fixture lists in
/// `starter_fixture(layers = ...)`; they are written like the fixture's own files, and a fixture
/// file with the same destination as a shared file panics.
///
/// Each fixture file drops its `.fixture` suffix. `MODULE.bazel` and `.bazelrc` are appended to
/// what `dx init` wrote; every other file is written as is. Returns the project root.
pub fn scaffold_fixture(scratch: &Path, fixture: &str, name: &str) -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let output = Command::new(dx_testing::resolve_runfiles(&rel))
        .current_dir(scratch)
        .args(["init", "--apply", name])
        .arg("--workspace")
        .arg(scratch)
        .output()
        .unwrap_or_else(|error| panic!("dx runs: {error}"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "dx init {name}: {text}");
    let root = scratch.join(name);
    let files = std::env::var("DX_FIXTURE_FILES")
        .unwrap_or_else(|_| panic!("DX_FIXTURE_FILES must list the {fixture} fixture"));
    let prefix = format!("{FIXTURES_DIR}/{fixture}/");
    let common = format!("{FIXTURES_DIR}/common/");
    let mut copied = 0;
    let mut own = Vec::new();
    let mut shared = Vec::new();
    for file in files.split_whitespace() {
        let rest = if let Some(rest) = file.strip_prefix(&prefix) {
            own.push(rest);
            rest
        } else if let Some(rest) = file.strip_prefix(&common) {
            let rest = rest
                .split_once('/')
                .unwrap_or_else(|| panic!("{file} is not under a common layer"))
                .1;
            shared.push(rest);
            rest
        } else {
            continue;
        };
        let dest = rest
            .strip_suffix(".fixture")
            .unwrap_or_else(|| panic!("{file} lacks the .fixture suffix"));
        let source = dx_testing::resolve_runfiles(file);
        let body =
            std::fs::read(&source).unwrap_or_else(|error| panic!("read {source:?}: {error}"));
        let target = root.join(dest);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
        }
        let body = if matches!(dest, "MODULE.bazel" | ".bazelrc") {
            let mut head = std::fs::read(&target).unwrap_or_default();
            if !head.is_empty() && !head.ends_with(b"\n") {
                head.push(b'\n');
            }
            head.push(b'\n');
            head.extend(body);
            head
        } else {
            body
        };
        std::fs::write(&target, body).unwrap_or_else(|error| panic!("write {target:?}: {error}"));
        copied += 1;
    }
    let duplicated: Vec<&&str> = own.iter().filter(|path| shared.contains(path)).collect();
    assert!(
        duplicated.is_empty(),
        "{fixture} repeats files of its common layers: {duplicated:?}"
    );
    assert!(
        copied > 0,
        "DX_FIXTURE_FILES lists no {fixture} files: {files}"
    );
    root
}
