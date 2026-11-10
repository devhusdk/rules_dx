//! How dx reads Bazel rc files and which disk cache and resource flags it adds.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use crate::{WORKFLOW_STARTUP_OPTS, is_ci_value};

pub(crate) const MAX_IMPORT_DEPTH: usize = 16;

/// The `--local_resources` values the vendored preset sets.
pub const PRESET_LOCAL_RESOURCES: [&str; 2] = ["cpu=HOST_CPUS*.75", "memory=HOST_RAM*.5"];

/// The `--experimental_disk_cache_gc_max_size` dx and the vendored preset set.
pub const DISK_CACHE_GC_MAX_SIZE: &str = "20G";

/// The `--experimental_disk_cache_gc_max_age` dx and the vendored preset set.
pub const DISK_CACHE_GC_MAX_AGE: &str = "14d";

const GC_MAX_SIZE_FLAG: &str = "--experimental_disk_cache_gc_max_size";
const GC_MAX_AGE_FLAG: &str = "--experimental_disk_cache_gc_max_age";
const UPLOAD_FLAG: &str = "--remote_upload_local_results";
const GC_IDLE_DELAY_FLAG: &str = "--experimental_disk_cache_gc_idle_delay";

/// Bazel's default `--experimental_disk_cache_gc_idle_delay`, in seconds.
const BAZEL_GC_IDLE_DELAY_SECS: u64 = 300;

/// The Bazel verbs dx adds `--local_resources` caps to.
const RESOURCE_VERBS: [&str; 4] = ["build", "test", "coverage", "run"];

/// A set of the verbs in `RESOURCE_VERBS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbMask(u8);

impl VerbMask {
    pub const ALL: VerbMask = VerbMask(0b1111);
    pub const NONE: VerbMask = VerbMask(0);

    /// Whether `verb` is in the set.
    pub fn contains(self, verb: &str) -> bool {
        RESOURCE_VERBS
            .iter()
            .position(|known| *known == verb)
            .is_some_and(|bit| self.0 & (1 << bit) != 0)
    }

    /// The set with `verb` added; other verbs are ignored.
    pub fn with(self, verb: &str) -> VerbMask {
        match RESOURCE_VERBS.iter().position(|known| *known == verb) {
            Some(bit) => VerbMask(self.0 | (1 << bit)),
            None => self,
        }
    }

    /// The verbs where the effective rc options do not already set `resource` to a non-preset value.
    fn where_rc_leaves_free(lines: &[sections::Line], resource: &str) -> VerbMask {
        RESOURCE_VERBS
            .iter()
            .filter(|verb| {
                let options = sections::effective_options(lines, verb, &[]);
                !rc_resource_in_effect(&options, resource)
            })
            .fold(VerbMask::NONE, |mask, verb| mask.with(verb))
    }

    /// The verbs where the effective rc options, without command-line configs, set `flag`.
    fn where_rc_sets(lines: &[sections::Line], flag: &str) -> VerbMask {
        RESOURCE_VERBS
            .iter()
            .filter(|verb| {
                sections::effective_options(lines, verb, &[])
                    .iter()
                    .any(|option| sets_flag(option, flag))
            })
            .fold(VerbMask::NONE, |mask, verb| mask.with(verb))
    }
}

/// Where the rc files set `--color`: plainly per verb, or once a named config is passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcColor {
    /// The verbs whose rc options set `--color` without any command-line `--config`.
    pub plain: VerbMask,
    /// Each named config and the verbs where expanding it sets `--color`.
    pub configs: Vec<(String, VerbMask)>,
}

impl RcColor {
    pub const NONE: RcColor = RcColor {
        plain: VerbMask::NONE,
        configs: Vec::new(),
    };

    fn from_lines(lines: &[sections::Line]) -> RcColor {
        let configs = sections::config_names(lines)
            .into_iter()
            .filter_map(|name| {
                let verbs = RESOURCE_VERBS
                    .iter()
                    .filter(|verb| {
                        sections::config_options(lines, verb, &name)
                            .iter()
                            .any(|option| sets_flag(option, "--color"))
                    })
                    .fold(VerbMask::NONE, |mask, verb| mask.with(verb));
                (verbs != VerbMask::NONE).then_some((name, verbs))
            })
            .collect();
        RcColor {
            plain: VerbMask::where_rc_sets(lines, "--color"),
            configs,
        }
    }

    /// Whether the rc files choose `--color` for the Bazel call `argv`.
    pub fn chosen(&self, argv: &[String]) -> bool {
        let Some(at) = argv.iter().skip(1).position(|arg| !arg.starts_with('-')) else {
            return false;
        };
        let verb = argv[at + 1].as_str();
        let verb = if RESOURCE_VERBS.contains(&verb) {
            verb
        } else {
            "build"
        };
        if self.plain.contains(verb) {
            return true;
        }
        let mut flags = argv[at + 2..].iter().take_while(|arg| *arg != "--");
        while let Some(flag) = flags.next() {
            let name = match flag.strip_prefix("--config=") {
                Some(name) => Some(name),
                None if flag == "--config" => flags.next().map(String::as_str),
                None => None,
            };
            if let Some(name) = name
                && self
                    .configs
                    .iter()
                    .any(|(config, verbs)| config == name && verbs.contains(verb))
            {
                return true;
            }
        }
        false
    }
}

/// Whether the last `resource` setting in `options` is not the preset default.
fn rc_resource_in_effect(options: &[String], resource: &str) -> bool {
    let mut in_effect = false;
    for (index, token) in options.iter().enumerate() {
        let next = options.get(index + 1).map(String::as_str);
        if !sets_local_resource(token, next, resource) {
            continue;
        }
        in_effect = !local_resources_value(token, next)
            .is_some_and(|value| PRESET_LOCAL_RESOURCES.contains(&value));
    }
    in_effect
}

/// The rc files dx lets Bazel read, the disk cache it adds, and which resource caps it may add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcPolicy {
    pub read_user_rc: bool,
    pub disk_cache: Option<String>,
    /// The disk cache a plain `build` uses: the rc value when an rc sets one, else dx's own.
    pub bazel_disk_cache: Option<PathBuf>,
    /// The verbs for which dx may add its `--local_resources=cpu` cap.
    pub local_cpu: VerbMask,
    /// The verbs for which dx may add its `--local_resources=memory` cap.
    pub local_memory: VerbMask,
    /// Whether dx bounds its disk cache with the gc max size and max age pair.
    pub disk_cache_gc: bool,
    /// The gc idle delay dx adds so cleanup can run before an idle server exits.
    pub gc_idle_delay_secs: Option<u64>,
    /// The `--max_idle_secs` dx starts a worktree slot's server with.
    pub slot_idle_secs: Option<u64>,
    /// Where an rc file sets `--color`, which then wins over `NO_COLOR`.
    pub rc_color: RcColor,
    /// Whether `DX_REMOTE_CACHE_READ_ONLY` is set, so dx adds `--noremote_upload_local_results`.
    pub remote_cache_read_only: bool,
}

static HERMETIC: RcPolicy = RcPolicy {
    read_user_rc: false,
    disk_cache: None,
    bazel_disk_cache: None,
    // CI and nested tests skip user rc files but still honor the slot share.
    local_cpu: VerbMask::ALL,
    local_memory: VerbMask::ALL,
    disk_cache_gc: false,
    gc_idle_delay_secs: None,
    slot_idle_secs: None,
    rc_color: RcColor::NONE,
    remote_cache_read_only: false,
};

static CURRENT: OnceLock<RcPolicy> = OnceLock::new();

type ShareFn = Arc<dyn Fn() -> LocalShare + Send + Sync>;

#[derive(Clone)]
enum ShareSource {
    Fixed(LocalShare),
    Live { token: u64, f: ShareFn },
}

static SHARE: RwLock<Option<ShareSource>> = RwLock::new(None);

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Identifies one registered live share source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareToken(u64);

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

/// Sets a fixed share this process passes to Bazel, replacing any live source.
pub fn set_share(share: Option<LocalShare>) {
    if let Ok(mut current) = SHARE.write() {
        *current = share.map(ShareSource::Fixed);
    }
}

/// Clears the share when it is the fixed value `share`.
pub fn clear_share(share: &LocalShare) {
    if let Ok(mut current) = SHARE.write()
        && matches!(current.as_ref(), Some(ShareSource::Fixed(fixed)) if fixed == share)
    {
        *current = None;
    }
}

/// Makes `share()` evaluate `f` on every call, replacing any earlier source.
pub fn set_share_source(f: impl Fn() -> LocalShare + Send + Sync + 'static) -> ShareToken {
    let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut current) = SHARE.write() {
        *current = Some(ShareSource::Live {
            token,
            f: Arc::new(f),
        });
    }
    ShareToken(token)
}

/// Removes the live source registered under `token`; a newer source stays.
pub fn clear_share_source(token: ShareToken) {
    if let Ok(mut current) = SHARE.write()
        && matches!(current.as_ref(), Some(ShareSource::Live { token: t, .. }) if *t == token.0)
    {
        *current = None;
    }
}

/// The share this process passes to Bazel, if any. A live source runs with no lock held.
pub fn share() -> Option<LocalShare> {
    let source = SHARE.read().ok().and_then(|current| current.clone())?;
    Some(match source {
        ShareSource::Fixed(share) => share,
        ShareSource::Live { f, .. } => f(),
    })
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
        Self::from_env_in(env_get, workspace, read_file, None)
    }

    /// `from_env` for a workspace whose server idles out after `slot_idle_secs`, when given.
    pub fn from_env_in(
        env_get: &dyn Fn(&str) -> Option<String>,
        workspace: &Path,
        read_file: &dyn Fn(&Path) -> Option<String>,
        slot_idle_secs: Option<u64>,
    ) -> RcPolicy {
        let in_bazel_test = env_get("BAZEL_TEST").is_some_and(|value| !value.is_empty());
        if in_bazel_test {
            return RcPolicy::hermetic();
        }
        let ci = is_ci_value(env_get("CI").as_deref());
        let lines = sections::load_rc(env_get, workspace, read_file, true, &[]);
        // Any rc line naming a disk cache flag, in any section, means dx adds no cache.
        let (disk_cache, bazel_disk_cache) =
            if sections::any_line_sets(&lines, &["--disk_cache", "--nodisk_cache"]) {
                let build = sections::effective_options(&lines, "build", &[]);
                let value = sections::last_disk_cache(&build).flatten();
                (
                    None,
                    value.map(|value| expand_path(value, env_get, workspace)),
                )
            } else if ci {
                (None, None)
            } else {
                let default = user_cache_dir(env_get).map(|cache| default_disk_cache(&cache));
                (
                    default
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned()),
                    default,
                )
            };
        let disk_cache_gc =
            disk_cache.is_some() && !rc_sets_disk_cache_gc(&lines);
        let slot_idle_secs = slot_idle_secs.filter(|secs| *secs > 0 && !ci);
        let gc_idle_delay_secs =
            if ci || sections::any_line_sets(&lines, &[GC_IDLE_DELAY_FLAG]) {
                None
            } else {
                let rc_idle = rc_min_max_idle_secs(&lines);
                let idle = match (rc_idle, slot_idle_secs) {
                    (Some(rc), Some(slot)) => Some(rc.min(slot)),
                    (rc, slot) => rc.or(slot),
                };
                idle.and_then(gc_idle_delay_for)
            };
        RcPolicy {
            read_user_rc: true,
            disk_cache,
            bazel_disk_cache,
            local_cpu: VerbMask::where_rc_leaves_free(&lines, "cpu"),
            local_memory: VerbMask::where_rc_leaves_free(&lines, "memory"),
            disk_cache_gc,
            gc_idle_delay_secs,
            slot_idle_secs,
            rc_color: RcColor::from_lines(&lines),
            remote_cache_read_only: is_ci_value(env_get("DX_REMOTE_CACHE_READ_ONLY").as_deref()),
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
        if self.remote_cache_read_only
            && !user_sets_flag(UPLOAD_FLAG)
            && !user_sets_flag("--noremote_upload_local_results")
        {
            options.push("--noremote_upload_local_results".to_owned());
        }
        if let Some(secs) = self.gc_idle_delay_secs
            && !user_sets_flag(GC_IDLE_DELAY_FLAG)
        {
            options.push(format!("{GC_IDLE_DELAY_FLAG}={secs}s"));
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
        if self.local_cpu.contains(bazel_command) && !user_sets("cpu") {
            options.push(format!("--local_resources=cpu={}", share.cpus));
        }
        if self.local_memory.contains(bazel_command) && !user_sets("memory") {
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
    local_resources_value(flag, next)
        .and_then(|value| value.strip_prefix(resource))
        .is_some_and(|rest| rest.starts_with('='))
}

/// Sets the policy for this process; the first call wins.
pub fn configure(policy: RcPolicy) {
    let _ = CURRENT.set(policy);
}

/// Sets the policy for this process from the real environment and files.
pub fn configure_real(workspace: &Path) {
    configure_real_in(workspace, None);
}

/// `configure_real` for a workspace whose server idles out after `slot_idle_secs`, when given.
pub fn configure_real_in(workspace: &Path, slot_idle_secs: Option<u64>) {
    configure(RcPolicy::from_env_in(
        &|name| std::env::var(name).ok(),
        workspace,
        &read_file,
        slot_idle_secs,
    ));
}

/// Reads an rc file the way Bazel does. A missing file is `None`; any other read error is
/// reported once per path and also read as `None`.
pub fn read_file(path: &Path) -> Option<String> {
    match std::fs::read(path) {
        Ok(bytes) => Some(decode(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            report_unreadable(path, &error);
            None
        }
    }
}

/// The rc text in `bytes`: invalid UTF-8 becomes U+FFFD and a leading byte-order mark is dropped.
fn decode(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned()
}

fn report_unreadable(path: &Path, error: &std::io::Error) {
    static REPORTED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
    let mut reported = REPORTED.lock().unwrap_or_else(PoisonError::into_inner);
    if reported.insert(path.to_path_buf()) {
        eprintln!("dx: cannot read rc file {}: {error}", path.display());
    }
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

/// The home directory Bazel expands `~` and finds the user rc in.
fn home_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    non_empty("HOME")
        .or_else(|| non_empty("USERPROFILE"))
        .map(PathBuf::from)
}

/// The system rc file: under `ProgramData` on Windows, else `/etc`.
pub(crate) fn system_rc(env_get: &dyn Fn(&str) -> Option<String>, windows: bool) -> Option<PathBuf> {
    if windows {
        env_get("ProgramData")
            .filter(|value| !value.is_empty())
            .map(|data| PathBuf::from(data).join("bazel.bazelrc"))
    } else {
        Some(PathBuf::from("/etc/bazel.bazelrc"))
    }
}

/// The repository cache directories Bazel may use, most likely first, without duplicates.
pub fn repository_caches(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    startup_options: &[String],
) -> Vec<PathBuf> {
    let lines = sections::load_rc(env_get, workspace, read_file, true, startup_options);
    let mut flagged = Vec::new();
    let mut rc_user_roots = Vec::new();
    for line in &lines {
        let tokens = line.tokens();
        for (index, token) in tokens.iter().enumerate() {
            let next = tokens.get(index + 1).map(String::as_str);
            if line.is_startup_any() {
                if let Some(value) = flag_value(token, next, "--output_user_root") {
                    rc_user_roots.push(value.to_owned());
                }
            } else if let Some(value) = flag_value(token, next, "--repository_cache") {
                flagged.push(value.to_owned());
            }
        }
    }
    let expand = |value: &str| expand_path(value, env_get, workspace);
    let mut candidates: Vec<PathBuf> = flagged
        .iter()
        .filter(|value| !value.is_empty())
        .map(|value| expand(value))
        .collect();
    let dx_user_roots = startup_options
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let next = startup_options.get(index + 1).map(String::as_str);
            flag_value(token, next, "--output_user_root").map(str::to_owned)
        });
    let user_roots: Vec<String> = dx_user_roots.chain(rc_user_roots).collect();
    for root in user_roots.iter().filter(|root| !root.is_empty()) {
        candidates.push(expand(root).join("cache").join("repos").join("v1"));
    }
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let user = non_empty("USER")
        .or_else(|| non_empty("LOGNAME"))
        .or_else(|| non_empty("USERNAME"));
    if let Some(user) = user {
        let mut bases: Vec<PathBuf> = Vec::new();
        bases.extend(non_empty("TEST_TMPDIR").map(PathBuf::from));
        bases.extend(non_empty("XDG_CACHE_HOME").map(|cache| PathBuf::from(cache).join("bazel")));
        if let Some(home) = home_dir(env_get) {
            bases.push(home.join(".cache").join("bazel"));
            bases.push(home.join("Library").join("Caches").join("bazel"));
        }
        bases.push(PathBuf::from("/private/var/tmp"));
        bases.extend(
            non_empty("USERPROFILE")
                .filter(|_| cfg!(windows))
                .map(PathBuf::from),
        );
        for base in bases {
            candidates.push(
                base.join(format!("_bazel_{user}"))
                    .join("cache")
                    .join("repos")
                    .join("v1"),
            );
        }
    }
    let mut seen = HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));
    candidates
}

/// Expands a leading `~` to the home directory and `%workspace%` to the workspace.
fn expand_path(value: &str, env_get: &dyn Fn(&str) -> Option<String>, workspace: &Path) -> PathBuf {
    let expanded = match (value.strip_prefix('~'), home_dir(env_get)) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(value),
    };
    import_path(&expanded.to_string_lossy(), workspace)
}

/// Whether any rc line sets the gc max size or max age.
fn rc_sets_disk_cache_gc(lines: &[sections::Line]) -> bool {
    sections::any_line_sets(lines, &[GC_MAX_SIZE_FLAG, GC_MAX_AGE_FLAG])
}

/// The smallest `startup --max_idle_secs` any rc line sets.
fn rc_min_max_idle_secs(lines: &[sections::Line]) -> Option<u64> {
    let mut least: Option<u64> = None;
    for line in lines.iter().filter(|line| line.is_startup()) {
        let tokens = line.tokens();
        for (index, token) in tokens.iter().enumerate() {
            let next = tokens.get(index + 1).map(String::as_str);
            let value = flag_value(token, next, "--max_idle_secs");
            if let Some(secs) = value.and_then(|value| value.parse::<u64>().ok()) {
                least = Some(least.map_or(secs, |least| least.min(secs)));
            }
        }
    }
    least
}

/// Splits an rc line on whitespace outside quotes, removing the quotes.
pub fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    for ch in line.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                in_token = true;
            }
            None if ch.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            None => {
                current.push(ch);
                in_token = true;
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

pub(crate) fn unquote(token: &str) -> &str {
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

/// Resolves an rc import against the workspace one component at a time, so a `\\?\` workspace
/// never ends up with a `/` separator inside it.
pub(crate) fn import_path(raw: &str, workspace: &Path) -> PathBuf {
    let relative = match raw.strip_prefix("%workspace%") {
        Some(rest) => rest,
        None if Path::new(raw).has_root() => return PathBuf::from(raw),
        None => raw,
    };
    let mut path = workspace.to_path_buf();
    path.extend(relative.split(['/', '\\']).filter(|part| !part.is_empty()));
    path
}

#[path = "rc_sections.rs"]
mod sections;

pub use sections::{
    StartupPaths, explicit_rc_files, nested_bazel_selected, remote_execution, startup_output_paths,
};

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // test asserts the launcher head
#[path = "rc_tests.rs"]
mod rc_tests;
