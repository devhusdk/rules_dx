//! Picks the shell Bazel runs genrules with on Windows.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const DOCS_URL: &str = "https://devhusdk.github.io/rules_dx/cli/commands/windows/#shell";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the first bash.exe on PATH is the WSL launcher ({0}) and Git bash was not found; \
     set BAZEL_SH to Git's usr\\bin\\bash.exe, see {DOCS_URL}"
)]
pub struct WslBash(pub PathBuf);

/// Returns the `BAZEL_SH` value to export, or `None` to leave the environment alone.
pub fn resolve_bazel_sh(
    get: &dyn Fn(&str) -> Option<OsString>,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<Option<PathBuf>, WslBash> {
    if get("BAZEL_SH").is_some_and(|value| !value.is_empty()) {
        return Ok(None);
    }
    let path = get("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    for dir in &dirs {
        if !exists(&dir.join("git.exe")) {
            continue;
        }
        let git_bash = dir
            .parent()
            .map(|root| root.join("usr").join("bin").join("bash.exe"))
            .filter(|bash| exists(bash));
        if git_bash.is_some() {
            return Ok(git_bash);
        }
    }
    let first = dirs
        .iter()
        .map(|dir| dir.join("bash.exe"))
        .find(|bash| exists(bash));
    let system_root = get("SystemRoot").filter(|root| !root.is_empty());
    match (first, system_root) {
        (Some(bash), Some(root)) if is_under(&bash, Path::new(&root)) => Err(WslBash(bash)),
        _ => Ok(None),
    }
}

fn is_under(path: &Path, root: &Path) -> bool {
    let lower = |p: &Path| p.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    let root = lower(root);
    let root = root.trim_end_matches('/');
    lower(path).starts_with(&format!("{root}/"))
}

#[cfg(test)]
#[path = "bazel_sh_tests.rs"]
mod bazel_sh_tests;
