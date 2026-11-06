//! Picks the shell Bazel runs genrules with on Windows.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const DOCS_URL: &str = "https://devhusdk.github.io/rules_dx/cli/commands/windows/#shell";

const UNIX_ROOT_ARG: &str = "-Dbazel.windows_unix_root=";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the first bash.exe on PATH is the WSL launcher ({0}) and Git bash was not found; \
     set BAZEL_SH to Git's usr\\bin\\bash.exe, see {DOCS_URL}"
)]
pub struct WslBash(pub PathBuf);

/// What to do with `BAZEL_SH` before running Bazel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BazelSh {
    Keep,
    Set(PathBuf),
    Unset,
}

/// Decides `BAZEL_SH` so a nested Bazel starts with the same bash as a manual `bazel` call.
pub fn resolve_bazel_sh(
    get: &dyn Fn(&str) -> Option<OsString>,
    exists: &dyn Fn(&Path) -> bool,
    read: &dyn Fn(&Path) -> Option<Vec<u8>>,
) -> Result<BazelSh, WslBash> {
    if let Some(value) = get("BAZEL_SH").filter(|value| !value.is_empty()) {
        let value = value.to_string_lossy().into_owned();
        let detected = server_unix_root(get, read).is_some_and(|root| root == value);
        return Ok(if detected && value.contains('\\') {
            BazelSh::Unset
        } else {
            BazelSh::Keep
        });
    }
    let path = get("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let first = dirs
        .iter()
        .map(|dir| dir.join("bash.exe"))
        .find(|bash| exists(bash));
    for dir in &dirs {
        if !exists(&dir.join("git.exe")) {
            continue;
        }
        let git_bash = dir
            .parent()
            .map(|root| root.join("usr").join("bin").join("bash.exe"))
            .filter(|bash| exists(bash));
        if let Some(bash) = git_bash {
            return Ok(if first.as_ref() == Some(&bash) {
                BazelSh::Keep
            } else {
                BazelSh::Set(bash)
            });
        }
    }
    let system_root = get("SystemRoot").filter(|root| !root.is_empty());
    match (first, system_root) {
        (Some(bash), Some(root)) if is_under(&bash, Path::new(&root)) => Err(WslBash(bash)),
        _ => Ok(BazelSh::Keep),
    }
}

fn server_unix_root(
    get: &dyn Fn(&str) -> Option<OsString>,
    read: &dyn Fn(&Path) -> Option<Vec<u8>>,
) -> Option<String> {
    let execroot = get("BUILD_EXECROOT").filter(|value| !value.is_empty())?;
    let output_base = crate::output_base_of_execroot(Path::new(&execroot))?;
    let cmdline = read(&output_base.join("server").join("cmdline"))?;
    cmdline
        .split(|byte| *byte == 0)
        .find_map(|arg| arg.strip_prefix(UNIX_ROOT_ARG.as_bytes()))
        .map(|root| String::from_utf8_lossy(root).into_owned())
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
