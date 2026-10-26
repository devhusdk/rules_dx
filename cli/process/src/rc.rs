//! How dx reads Bazel rc files and which disk cache and resource flags it adds.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use crate::{is_ci_value, WORKFLOW_STARTUP_OPTS};

const MAX_IMPORT_DEPTH: usize = 16;

/// The `--local_resources` values the vendored preset sets.
pub const PRESET_LOCAL_RESOURCES: [&str; 2] = ["cpu=HOST_CPUS*.75", "memory=HOST_RAM*.5"];

/// The `--experimental_disk_cache_gc_max_size` dx and the vendored preset set.
pub const DISK_CACHE_GC_MAX_SIZE: &str = "20G";

/// The `--experimental_disk_cache_gc_max_age` dx and the vendored preset set.
pub const DISK_CACHE_GC_MAX_AGE: &str = "14d";

const GC_MAX_SIZE_FLAG: &str = "--experimental_disk_cache_gc_max_size";
const GC_MAX_AGE_FLAG: &str = "--experimental_disk_cache_gc_max_age";
const GC_IDLE_DELAY_FLAG: &str = "--experimental_disk_cache_gc_idle_delay";

/// Bazel's default `--experimental_disk_cache_gc_idle_delay`, in seconds.
const BAZEL_GC_IDLE_DELAY_SECS: u64 = 300;

/// The rc files dx lets Bazel read, the disk cache it adds, and which resource caps it may add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcPolicy {
    pub read_user_rc: bool,
    pub disk_cache: Option<String>,
    pub local_cpu: bool,
    pub local_memory: bool,
    /// Whether dx bounds its disk cache with the gc max size and max age pair.
    pub disk_cache_gc: bool,
    /// The gc idle delay dx adds so cleanup can run before an idle server exits.
    pub gc_idle_delay_secs: Option<u64>,
}

static HERMETIC: RcPolicy = RcPolicy {
    read_user_rc: false,
    disk_cache: None,
    local_cpu: false,
    local_memory: false,
    disk_cache_gc: false,
    gc_idle_delay_secs: None,
};

static CURRENT: OnceLock<RcPolicy> = OnceLock::new();

static SHARE: RwLock<Option<LocalShare>> = RwLock::new(None);

/// The CPUs and memory one build slot may plan for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalShare {
    pub cpus: u64,
    pub memory_mb: u64,
}

impl LocalShare {
    /// The share as `cpu=N,memory=M`.
    pub fn to_env(&self) -> String {
        format!("cpu={},memory={}", self.cpus, self.memory_mb)
    }

    /// Parses `cpu=N,memory=M`; anything else is `None`.
    pub fn parse(text: &str) -> Option<LocalShare> {
        let (cpu, memory) = text.trim().split_once(',')?;
        let cpus = cpu.strip_prefix("cpu=")?.parse().ok()?;
        let memory_mb = memory.strip_prefix("memory=")?.parse().ok()?;
        (cpus > 0 && memory_mb > 0).then_some(LocalShare { cpus, memory_mb })
    }
}

/// Sets the share this process passes to Bazel.
pub fn set_share(share: Option<LocalShare>) {
    if let Ok(mut current) = SHARE.write() {
        *current = share;
    }
}

/// Clears the share when it is still `share`.
pub fn clear_share(share: &LocalShare) {
    if let Ok(mut current) = SHARE.write() {
        if current.as_ref() == Some(share) {
            *current = None;
        }
    }
}

/// The share this process passes to Bazel, if any.
pub fn share() -> Option<LocalShare> {
    SHARE.read().ok().and_then(|current| *current)
}

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
        let disk_cache_gc =
            disk_cache.is_some() && !rc_sets_disk_cache_gc(&roots, workspace, read_file);
        let gc_idle_delay_secs = if rc_sets_flag(&roots, workspace, read_file, GC_IDLE_DELAY_FLAG) {
            None
        } else {
            rc_min_max_idle_secs(&roots, workspace, read_file).and_then(gc_idle_delay_for)
        };
        RcPolicy {
            read_user_rc: true,
            disk_cache,
            local_cpu: !rc_sets_local_resource(&roots, workspace, read_file, "cpu"),
            local_memory: !rc_sets_local_resource(&roots, workspace, read_file, "memory"),
            disk_cache_gc,
            gc_idle_delay_secs,
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

    /// The cache, cache gc, and resource flags added right after a build, test, coverage, or run verb.
    pub fn verb_options(
        &self,
        bazel_command: &str,
        share: Option<&LocalShare>,
        user_options: &[String],
    ) -> Vec<String> {
        if !matches!(bazel_command, "build" | "test" | "coverage" | "run") {
            return Vec::new();
        }
        let user_sets_flag = |name: &str| {
            user_options
                .iter()
                .any(|token| sets_flag(token.trim_start_matches(['"', '\'']), name))
        };
        let mut options = Vec::new();
        if let Some(path) = &self.disk_cache {
            options.push(format!("--disk_cache={path}"));
            let user_sets_cache =
                user_sets_flag("--disk_cache") || user_sets_flag("--nodisk_cache");
            if self.disk_cache_gc
                && !user_sets_cache
                && !user_sets_flag(GC_MAX_SIZE_FLAG)
                && !user_sets_flag(GC_MAX_AGE_FLAG)
            {
                options.push(format!("{GC_MAX_SIZE_FLAG}={DISK_CACHE_GC_MAX_SIZE}"));
                options.push(format!("{GC_MAX_AGE_FLAG}={DISK_CACHE_GC_MAX_AGE}"));
            }
        }
        if let Some(secs) = self.gc_idle_delay_secs {
            if !user_sets_flag(GC_IDLE_DELAY_FLAG) {
                options.push(format!("{GC_IDLE_DELAY_FLAG}={secs}s"));
            }
        }
        let Some(share) = share else {
            return options;
        };
        let user_sets = |resource: &str| {
            user_options.iter().enumerate().any(|(index, token)| {
                let next = user_options.get(index + 1).map(String::as_str);
                sets_local_resource(token, next, resource)
            })
        };
        if self.local_cpu && !user_sets("cpu") {
            options.push(format!("--local_resources=cpu={}", share.cpus));
        }
        if self.local_memory && !user_sets("memory") {
            options.push(format!("--local_resources=memory={}", share.memory_mb));
        }
        options
    }
}

/// The value a flag token gives `name`, reading the next token for the split form.
fn flag_value<'a>(flag: &'a str, next: Option<&'a str>, name: &str) -> Option<&'a str> {
    match flag.strip_prefix(name)? {
        "" => next,
        rest => rest.strip_prefix('='),
    }
}

/// Whether a flag token is exactly `name`, alone or followed by `=`.
fn sets_flag(flag: &str, name: &str) -> bool {
    flag.strip_prefix(name)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
}

/// The gc idle delay that fits under `max_idle_secs`, or `None` when Bazel's default already does.
fn gc_idle_delay_for(max_idle_secs: u64) -> Option<u64> {
    let delay = max_idle_secs / 3;
    (max_idle_secs > 0 && delay < BAZEL_GC_IDLE_DELAY_SECS).then_some(delay.max(1))
}

/// The `--local_resources` value a flag token sets, reading the next token for the split form.
fn local_resources_value<'a>(flag: &'a str, next: Option<&'a str>) -> Option<&'a str> {
    flag_value(flag, next, "--local_resources")
}

/// Whether a flag token, with the token after it, sets `resource`.
fn sets_local_resource(token: &str, next: Option<&str>, resource: &str) -> bool {
    let flag = token.trim_start_matches(['"', '\'']);
    let legacy = match resource {
        "cpu" => Some("--local_cpu_resources"),
        "memory" => Some("--local_ram_resources"),
        _ => None,
    };
    if legacy.is_some_and(|legacy| flag.split('=').next() == Some(legacy)) {
        return true;
    }
    local_resources_value(flag, next)
        .and_then(|value| value.strip_prefix(resource))
        .is_some_and(|rest| rest.starts_with('='))
}

/// Whether a flag token sets `resource` to anything but the preset default.
fn overrides_preset(token: &str, next: Option<&str>, resource: &str) -> bool {
    let flag = token.trim_start_matches(['"', '\'']);
    sets_local_resource(flag, next, resource)
        && !local_resources_value(flag, next)
            .is_some_and(|value| PRESET_LOCAL_RESOURCES.contains(&value))
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

/// The cache and resource flags this process adds after `bazel_command`.
pub fn build_verb_options(bazel_command: &str, user_options: &[String]) -> Vec<String> {
    current().verb_options(bazel_command, share().as_ref(), user_options)
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
    let sets = |token: &str, _: Option<&str>| {
        token.starts_with("--disk_cache") || token.starts_with("--nodisk_cache")
    };
    scan_roots(roots, workspace, read_file, &sets)
}

/// Whether any rc file reachable from `roots` sets the gc max size or max age.
pub fn rc_sets_disk_cache_gc(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
) -> bool {
    rc_sets_flag(roots, workspace, read_file, GC_MAX_SIZE_FLAG)
        || rc_sets_flag(roots, workspace, read_file, GC_MAX_AGE_FLAG)
}

fn rc_sets_flag(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    name: &str,
) -> bool {
    let sets = |token: &str, _: Option<&str>| sets_flag(token, name);
    scan_roots(roots, workspace, read_file, &sets)
}

/// The smallest `startup --max_idle_secs` any rc file reachable from `roots` sets.
fn rc_min_max_idle_secs(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
) -> Option<u64> {
    let mut least: Option<u64> = None;
    let mut visit = |tokens: &[&str]| {
        if tokens.first() != Some(&"startup") {
            return false;
        }
        for (index, token) in tokens.iter().enumerate() {
            let flag = token.trim_start_matches(['"', '\'']);
            let value = flag_value(flag, tokens.get(index + 1).copied(), "--max_idle_secs");
            if let Some(secs) = value.and_then(|value| value.parse::<u64>().ok()) {
                least = Some(least.map_or(secs, |least| least.min(secs)));
            }
        }
        false
    };
    visit_roots(roots, workspace, read_file, &mut visit);
    least
}

/// Whether any rc file reachable from `roots` sets `resource` to anything but the preset default.
pub fn rc_sets_local_resource(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    resource: &str,
) -> bool {
    let sets = |token: &str, next: Option<&str>| overrides_preset(token, next, resource);
    scan_roots(roots, workspace, read_file, &sets)
}

type Matcher<'a> = dyn Fn(&str, Option<&str>) -> bool + 'a;

/// Visits the tokens of each rc line; returning `true` stops the walk.
type LineVisitor<'a> = dyn FnMut(&[&str]) -> bool + 'a;

fn scan_roots(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    matches: &Matcher<'_>,
) -> bool {
    let mut visit = |tokens: &[&str]| {
        tokens.iter().enumerate().any(|(index, token)| {
            let flag = token.trim_start_matches(['"', '\'']);
            matches(flag, tokens.get(index + 1).copied())
        })
    };
    visit_roots(roots, workspace, read_file, &mut visit)
}

fn visit_roots(
    roots: &[PathBuf],
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    visit: &mut LineVisitor<'_>,
) -> bool {
    let mut seen = HashSet::new();
    roots
        .iter()
        .any(|root| scan(root, workspace, read_file, visit, &mut seen, 0))
}

fn scan(
    path: &Path,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    visit: &mut LineVisitor<'_>,
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
        let tokens: Vec<&str> = line.split_whitespace().map(unquote).collect();
        let first = tokens.first().copied().unwrap_or_default();
        if matches!(first, "import" | "try-import") {
            let rest = line[first.len()..].trim();
            let target = import_path(unquote(rest), workspace);
            if scan(&target, workspace, read_file, visit, seen, depth + 1) {
                return true;
            }
            continue;
        }
        if visit(&tokens) {
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
