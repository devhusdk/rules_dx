//! How dx reads Bazel rc files and which disk cache it adds.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::{is_ci_value, WORKFLOW_STARTUP_OPTS};

const MAX_IMPORT_DEPTH: usize = 16;

/// The rc files dx lets Bazel read and the disk cache it adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcPolicy {
    pub read_user_rc: bool,
    pub disk_cache: Option<String>,
}

static HERMETIC: RcPolicy = RcPolicy {
    read_user_rc: false,
    disk_cache: None,
};

static CURRENT: OnceLock<RcPolicy> = OnceLock::new();

impl RcPolicy {
    /// The policy that skips user rc files and adds no disk cache.
    pub fn hermetic() -> RcPolicy {
        HERMETIC.clone()
    }

    /// Resolves the policy from the environment and the rc files Bazel would read.
    pub fn from_env(
        env_get: &dyn Fn(&str) -> Option<String>,
        workspace: &Path,
        read_file: &dyn Fn(&Path) -> Option<String>,
    ) -> RcPolicy {
        let in_bazel_test = env_get("BAZEL_TEST").is_some_and(|value| !value.is_empty());
        if is_ci_value(env_get("CI").as_deref()) || in_bazel_test {
            return RcPolicy::hermetic();
        }
        let roots = rc_roots(env_get, workspace);
        let disk_cache = if rc_sets_disk_cache(&roots, workspace, read_file) {
            None
        } else {
            user_cache_dir(env_get)
                .map(|cache| default_disk_cache(&cache).to_string_lossy().into_owned())
        };
        RcPolicy {
            read_user_rc: true,
            disk_cache,
        }
    }

    /// The startup flags every Bazel call starts with.
    pub fn startup_opts(&self) -> &'static [&'static str] {
        if self.read_user_rc {
            &[]
        } else {
            WORKFLOW_STARTUP_OPTS
        }
    }

    /// The cache flags added right after a build, test, coverage, or run verb.
    pub fn cache_options(&self, bazel_command: &str) -> Vec<String> {
        match &self.disk_cache {
            Some(path) if matches!(bazel_command, "build" | "test" | "coverage" | "run") => {
                vec![format!("--disk_cache={path}")]
            }
            _ => Vec::new(),
        }
    }
}

/// Sets the policy for this process; the first call wins.
pub fn configure(policy: RcPolicy) {
    let _ = CURRENT.set(policy);
}

/// Sets the policy for this process from the real environment and files.
pub fn configure_real(workspace: &Path) {
    configure(RcPolicy::from_env(
        &|name| std::env::var(name).ok(),
        workspace,
        &|path| std::fs::read_to_string(path).ok(),
    ));
}

/// The policy for this process, hermetic until configured.
pub fn current() -> &'static RcPolicy {
    CURRENT.get().unwrap_or(&HERMETIC)
}

/// The cache flags this process adds after `bazel_command`.
pub fn build_cache_options(bazel_command: &str) -> Vec<String> {
    current().cache_options(bazel_command)
}

/// The user cache directory: XDG_CACHE_HOME, then HOME/.cache, then LOCALAPPDATA.
pub fn user_cache_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    match non_empty("XDG_CACHE_HOME") {
        Some(cache) => Some(PathBuf::from(cache)),
        None => match non_empty("HOME") {
            Some(home) => Some(PathBuf::from(home).join(".cache")),
            None => non_empty("LOCALAPPDATA").map(PathBuf::from),
        },
    }
}

/// The disk cache dx shares across every checkout under `cache`.
pub fn default_disk_cache(cache: &Path) -> PathBuf {
    cache.join("dx").join("disk-cache")
}

fn rc_roots(env_get: &dyn Fn(&str) -> Option<String>, workspace: &Path) -> Vec<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let mut roots = vec![workspace.join(".bazelrc")];
    if let Some(home) = non_empty("HOME").or_else(|| non_empty("USERPROFILE")) {
        roots.push(PathBuf::from(home).join(".bazelrc"));
    }
    if cfg!(windows) {
        if let Some(data) = non_empty("ProgramData") {
            roots.push(PathBuf::from(data).join("bazel.bazelrc"));
        }
    } else {
        roots.push(PathBuf::from("/etc/bazel.bazelrc"));
    }
    roots
}

/// Whether any rc file reachable from `roots` mentions a disk cache flag.
pub fn rc_sets_disk_cache(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
) -> bool {
    let mut seen = HashSet::new();
    roots
        .iter()
        .any(|root| scan(root, workspace, read_file, &mut seen, 0))
}

fn scan(
    path: &Path,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    seen: &mut HashSet<PathBuf>,
    depth: usize,
) -> bool {
    if depth > MAX_IMPORT_DEPTH || !seen.insert(path.to_path_buf()) {
        return false;
    }
    let Some(text) = read_file(path) else {
        return false;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace().map(unquote);
        let first = tokens.next().unwrap_or_default();
        if matches!(first, "import" | "try-import") {
            let rest = line[first.len()..].trim();
            let target = import_path(unquote(rest), workspace);
            if scan(&target, workspace, read_file, seen, depth + 1) {
                return true;
            }
            continue;
        }
        if std::iter::once(first).chain(tokens).any(|token| {
            let flag = token.trim_start_matches(['"', '\'']);
            flag.starts_with("--disk_cache") || flag.starts_with("--nodisk_cache")
        }) {
            return true;
        }
    }
    false
}

fn unquote(token: &str) -> &str {
    token
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            token
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(token)
}

fn import_path(raw: &str, workspace: &Path) -> PathBuf {
    let expanded = raw.replace("%workspace%", &workspace.to_string_lossy());
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

#[cfg(test)]
#[path = "rc_tests.rs"]
mod rc_tests;
