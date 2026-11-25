use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use crate::{WORKFLOW_STARTUP_OPTS, is_ci_value};

pub(crate) const MAX_IMPORT_DEPTH: usize = 16;

pub const PRESET_LOCAL_RESOURCES: [&str; 2] = ["cpu=HOST_CPUS*.75", "memory=HOST_RAM*.25"];

pub const DISK_CACHE_GC_MAX_SIZE: &str = "20G";

pub const DISK_CACHE_GC_MAX_AGE: &str = "14d";

const GC_MAX_SIZE_FLAG: &str = "--experimental_disk_cache_gc_max_size";
const GC_MAX_AGE_FLAG: &str = "--experimental_disk_cache_gc_max_age";
const UPLOAD_FLAG: &str = "--remote_upload_local_results";
const GC_IDLE_DELAY_FLAG: &str = "--experimental_disk_cache_gc_idle_delay";
const PROFILE_FLAG: &str = "--profile";

const PROFILE_DIR_ENV: &str = "DX_PROFILE_DIR";

static PROFILE_COUNTER: AtomicU64 = AtomicU64::new(0);

const BAZEL_GC_IDLE_DELAY_SECS: u64 = 300;

const RESOURCE_VERBS: [&str; 4] = ["build", "test", "coverage", "run"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbMask(u8);

impl VerbMask {
    pub const ALL: VerbMask = VerbMask(0b1111);
    pub const NONE: VerbMask = VerbMask(0);

    #[must_use]
    pub fn contains(self, verb: &str) -> bool {
        RESOURCE_VERBS
            .iter()
            .position(|known| *known == verb)
            .is_some_and(|bit| self.0 & (1 << bit) != 0)
    }

    #[must_use]
    pub fn with(self, verb: &str) -> VerbMask {
        match RESOURCE_VERBS.iter().position(|known| *known == verb) {
            Some(bit) => VerbMask(self.0 | (1 << bit)),
            None => self,
        }
    }

    fn where_rc_leaves_free(lines: &[sections::Line], resource: &str) -> VerbMask {
        RESOURCE_VERBS
            .iter()
            .filter(|verb| {
                let options = sections::effective_options(lines, verb, &[]);
                !rc_resource_in_effect(&options, resource)
            })
            .fold(VerbMask::NONE, |mask, verb| mask.with(verb))
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcColor {
    pub plain: VerbMask,
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

    pub fn chosen(&self, argv: &[String]) -> bool {
        let Some((at, verb)) = argv
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, arg)| !arg.starts_with('-'))
        else {
            return false;
        };
        let verb = verb.as_str();
        let verb = if RESOURCE_VERBS.contains(&verb) {
            verb
        } else {
            "build"
        };
        if self.plain.contains(verb) {
            return true;
        }
        let mut flags = argv.iter().skip(at + 1).take_while(|arg| *arg != "--");
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcPolicy {
    pub read_user_rc: bool,
    pub disk_cache: Option<String>,
    pub bazel_disk_cache: Option<PathBuf>,
    pub local_cpu: VerbMask,
    pub local_memory: VerbMask,
    pub disk_cache_gc: bool,
    pub gc_idle_delay_secs: Option<u64>,
    pub slot_idle_secs: Option<u64>,
    pub rc_color: RcColor,
    pub remote_cache_read_only: bool,
    pub profile_dir: Option<PathBuf>,
}

static HERMETIC: RcPolicy = RcPolicy {
    read_user_rc: false,
    disk_cache: None,
    bazel_disk_cache: None,
    local_cpu: VerbMask::ALL,
    local_memory: VerbMask::ALL,
    disk_cache_gc: false,
    gc_idle_delay_secs: None,
    slot_idle_secs: None,
    rc_color: RcColor::NONE,
    remote_cache_read_only: false,
    profile_dir: None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareToken(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalShare {
    pub cpus: u64,
    pub memory_mb: u64,
}

impl LocalShare {
    #[must_use]
    pub fn to_env(&self) -> String {
        format!("cpu={},memory={}", self.cpus, self.memory_mb)
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<LocalShare> {
        let (cpu, memory) = text.trim().split_once(',')?;
        let cpus = cpu.strip_prefix("cpu=")?.parse().ok()?;
        let memory_mb = memory.strip_prefix("memory=")?.parse().ok()?;
        (cpus > 0 && memory_mb > 0).then_some(LocalShare { cpus, memory_mb })
    }
}

pub fn set_share(share: Option<LocalShare>) {
    if let Ok(mut current) = SHARE.write() {
        *current = share.map(ShareSource::Fixed);
    }
}

pub fn clear_share(share: &LocalShare) {
    if let Ok(mut current) = SHARE.write()
        && matches!(current.as_ref(), Some(ShareSource::Fixed(fixed)) if fixed == share)
    {
        *current = None;
    }
}

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

pub fn clear_share_source(token: ShareToken) {
    if let Ok(mut current) = SHARE.write()
        && matches!(current.as_ref(), Some(ShareSource::Live { token: t, .. }) if *t == token.0)
    {
        *current = None;
    }
}

pub fn share() -> Option<LocalShare> {
    let source = SHARE.read().ok().and_then(|current| current.clone())?;
    Some(match source {
        ShareSource::Fixed(share) => share,
        ShareSource::Live { f, .. } => f(),
    })
}

impl RcPolicy {
    #[must_use]
    pub fn hermetic() -> RcPolicy {
        HERMETIC.clone()
    }

    pub fn from_env(
        env_get: &dyn Fn(&str) -> Option<String>,
        workspace: &Path,
        read_file: &dyn Fn(&Path) -> Option<String>,
    ) -> RcPolicy {
        Self::from_env_in(env_get, workspace, read_file, None)
    }

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
        let disk_cache_gc = disk_cache.is_some() && !rc_sets_disk_cache_gc(&lines);
        let slot_idle_secs = slot_idle_secs.filter(|secs| *secs > 0 && !ci);
        let gc_idle_delay_secs = if ci || sections::any_line_sets(&lines, &[GC_IDLE_DELAY_FLAG]) {
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
            profile_dir: env_get(PROFILE_DIR_ENV)
                .filter(|dir| !dir.is_empty())
                .map(|dir| expand_path(&dir, env_get, workspace)),
        }
    }

    pub fn ensure_profile_dir(&self) -> Result<(), String> {
        let Some(dir) = &self.profile_dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir).map_err(|error| {
            format!(
                "cannot create {PROFILE_DIR_ENV} directory {}: {error}",
                dir.display()
            )
        })
    }

    #[must_use]
    pub fn startup_opts(&self) -> &'static [&'static str] {
        if self.read_user_rc {
            &[]
        } else {
            WORKFLOW_STARTUP_OPTS
        }
    }

    #[must_use]
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
        if let Some(dir) = &self.profile_dir
            && !user_sets_flag(PROFILE_FLAG)
        {
            let trace = dir.join(profile_name(bazel_command));
            options.push(format!("{PROFILE_FLAG}={}", trace.display()));
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

fn flag_value<'a>(flag: &'a str, next: Option<&'a str>, name: &str) -> Option<&'a str> {
    match flag.strip_prefix(name)? {
        "" => next,
        rest => rest.strip_prefix('='),
    }
}

fn sets_flag(flag: &str, name: &str) -> bool {
    flag.strip_prefix(name)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
}

fn profile_name(bazel_command: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let call = PROFILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{millis}-{}-{call}-{bazel_command}.json.gz",
        std::process::id()
    )
}

fn gc_idle_delay_for(max_idle_secs: u64) -> Option<u64> {
    let delay = max_idle_secs / 3;
    (max_idle_secs > 0 && delay < BAZEL_GC_IDLE_DELAY_SECS).then_some(delay.max(1))
}

fn local_resources_value<'a>(flag: &'a str, next: Option<&'a str>) -> Option<&'a str> {
    flag_value(flag, next, "--local_resources")
}

fn sets_local_resource(token: &str, next: Option<&str>, resource: &str) -> bool {
    let flag = token.trim_start_matches(['"', '\'']);
    local_resources_value(flag, next)
        .and_then(|value| value.strip_prefix(resource))
        .is_some_and(|rest| rest.starts_with('='))
}

pub fn configure(policy: RcPolicy) {
    drop(CURRENT.set(policy));
}

pub fn configure_real(workspace: &Path) {
    configure_real_in(workspace, None);
}

pub fn configure_real_in(workspace: &Path, slot_idle_secs: Option<u64>) {
    configure(RcPolicy::from_env_in(
        &|name| std::env::var(name).ok(),
        workspace,
        &read_file,
        slot_idle_secs,
    ));
}

#[must_use]
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

fn decode(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned()
}

fn report_unreadable(path: &Path, error: &std::io::Error) {
    static REPORTED: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
    let mut reported = REPORTED.lock().unwrap_or_else(PoisonError::into_inner);
    if reported.insert(path.to_path_buf()) {
        dx_error::stdio::eprint_line(format_args!(
            "dx: cannot read rc file {}: {error}",
            path.display()
        ));
    }
}

pub fn current() -> &'static RcPolicy {
    CURRENT.get().unwrap_or(&HERMETIC)
}

#[must_use]
pub fn build_verb_options(bazel_command: &str, user_options: &[String]) -> Vec<String> {
    current().verb_options(bazel_command, share().as_ref(), user_options)
}

pub fn user_cache_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    // has_root, not is_absolute, so a rooted `/x` still counts on Windows.
    let xdg = non_empty("XDG_CACHE_HOME").filter(|cache| Path::new(cache).has_root());
    match xdg {
        Some(cache) => Some(PathBuf::from(cache)),
        None => match non_empty("HOME") {
            Some(home) => Some(PathBuf::from(home).join(".cache")),
            None => non_empty("LOCALAPPDATA").map(PathBuf::from),
        },
    }
}

#[must_use]
pub fn default_disk_cache(cache: &Path) -> PathBuf {
    cache.join("dx").join("disk-cache")
}

fn home_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    crate::home_with(env_get).map(PathBuf::from)
}

pub(crate) fn system_rc(
    env_get: &dyn Fn(&str) -> Option<String>,
    windows: bool,
) -> Option<PathBuf> {
    if windows {
        env_get("ProgramData")
            .filter(|value| !value.is_empty())
            .map(|data| PathBuf::from(data).join("bazel.bazelrc"))
    } else {
        Some(PathBuf::from("/etc/bazel.bazelrc"))
    }
}

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

pub fn downloader_config(
    env_get: &dyn Fn(&str) -> Option<String>,
    workspace: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
    read_user_rc: bool,
    startup_options: &[String],
    options: &[String],
) -> Option<PathBuf> {
    let lines = sections::load_rc(env_get, workspace, read_file, read_user_rc, startup_options);
    let effective = sections::effective_options(&lines, "run", options);
    let mut last = None;
    for (index, token) in effective.iter().enumerate() {
        let next = effective.get(index + 1).map(String::as_str);
        if let Some(value) = flag_value(token, next, "--downloader_config")
            .or_else(|| flag_value(token, next, "--experimental_downloader_config"))
        {
            last = Some(value);
        }
    }
    last.map(unquote)
        .filter(|value| !value.is_empty())
        .map(|value| import_path(value, workspace))
}

fn expand_path(value: &str, env_get: &dyn Fn(&str) -> Option<String>, workspace: &Path) -> PathBuf {
    let expanded = match (value.strip_prefix('~'), home_dir(env_get)) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(value),
    };
    import_path(&expanded.to_string_lossy(), workspace)
}

fn rc_sets_disk_cache_gc(lines: &[sections::Line]) -> bool {
    sections::any_line_sets(lines, &[GC_MAX_SIZE_FLAG, GC_MAX_AGE_FLAG])
}

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

#[must_use]
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
#[expect(clippy::disallowed_methods, reason = "test asserts the launcher head")]
#[path = "rc_tests.rs"]
mod rc_tests;
