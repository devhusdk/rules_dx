//! Starts a project with `dx init` and adds a hello-world fixture.
#![deny(clippy::map_err_ignore, clippy::unused_result_ok)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The runfiles directory that holds the hello-world fixtures, one subdirectory per language.
pub const FIXTURES_DIR: &str = "tools/testing/starters/fixtures";

/// Starts `scratch/<name>` the way a user does: `dx init --apply --no-generate <name>` with the dx named by
/// `DX_BIN`, then the hello-world sources of `fixture`, read from the runfiles that `files` lists
/// (`$(rootpaths //tools/testing/starters/fixtures:<fixture>)`; files of other fixtures in the
/// list are skipped).
///
/// Files under `common/<layer>/` of the fixtures directory are shared layers the fixture lists in
/// `starter_fixture(layers = ...)`; they are written like the fixture's own files, and a fixture
/// file with the same destination as a shared file is an error.
///
/// Each fixture file drops its `.fixture` suffix. `MODULE.bazel` is appended to
/// what `dx init` wrote; every other file is written as is. Returns the project root.
///
/// # Errors
///
/// Returns `Err` when `dx init` fails or a fixture file cannot be read or written.
pub fn scaffold_fixture(
    scratch: &Path,
    fixture: &str,
    name: &str,
    files: &str,
) -> Result<PathBuf, String> {
    let rel = std::env::var("DX_BIN").map_err(|error| format!("DX_BIN must name dx: {error}"))?;
    let output = Command::new(dx_testing::resolve_runfiles(&rel))
        .current_dir(scratch)
        .args(["init", "--apply", "--no-generate", name])
        .arg("--workspace")
        .arg(scratch)
        .output()
        .map_err(|error| format!("dx runs: {error}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.code() != Some(0) {
        return Err(format!("dx init {name}: {text}"));
    }
    let root = scratch.join(name);
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
            let (_, rest) = rest
                .split_once('/')
                .ok_or_else(|| format!("{file} is not under a common layer"))?;
            shared.push(rest);
            rest
        } else {
            continue;
        };
        let dest = rest
            .strip_suffix(".fixture")
            .ok_or_else(|| format!("{file} lacks the .fixture suffix"))?;
        let source = dx_testing::resolve_runfiles(file);
        let body = std::fs::read(&source)
            .map_err(|error| format!("read {}: {error}", source.display()))?;
        let target = root.join(dest);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("mkdir {}: {error}", parent.display()))?;
        }
        let body = if dest == "MODULE.bazel" {
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
        std::fs::write(&target, body)
            .map_err(|error| format!("write {}: {error}", target.display()))?;
        copied += 1;
    }
    let duplicated: Vec<&&str> = own.iter().filter(|path| shared.contains(path)).collect();
    if !duplicated.is_empty() {
        return Err(format!(
            "{fixture} repeats files of its common layers: {duplicated:?}"
        ));
    }
    if copied == 0 {
        return Err(format!("no {fixture} files in the list: {files}"));
    }
    Ok(root)
}
