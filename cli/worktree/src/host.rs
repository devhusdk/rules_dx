use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_process::lifecycle::{
    CapturePolicy, ChildOutcome, CleanupBudget, Descendants, EnvPolicy, SpawnSpec, run_within_until,
};

/// One running Bazel server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub pid: u32,
    pub rss_bytes: u64,
    pub workspace: Option<PathBuf>,
    pub output_base: Option<PathBuf>,
    pub output_user_root: Option<PathBuf>,
}

/// One Bazel output base and the workspace it was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
}

pub(crate) use dx_process::DiskSpace;

/// Names the slot count above which `add` deletes surplus free slots; `0` turns that off.
pub const MAX_SLOTS_ENV: &str = "DX_WORKTREE_MAX_SLOTS";
const DEFAULT_MAX_SLOTS: usize = 24;

/// The machine facts the pool reads and the effects it has outside git.
pub trait Host {
    fn now(&self) -> u64;
    fn process_alive(&self, pid: u32) -> bool;

    /// The start time of `pid`, when the OS gives one.
    fn process_start(&self, pid: u32) -> Option<String>;

    /// The id of the current boot, when the OS gives one.
    fn boot_id(&self) -> Option<String>;

    /// Whether a running process has `dir` or a directory below it as its working directory.
    fn process_in(&self, _dir: &Path) -> bool {
        false
    }
    fn disk_space(&self, path: &Path) -> Option<DiskSpace>;
    fn servers(&self) -> Vec<Server>;
    /// Whether `servers` can see the process list; false when `ps` is absent or fails.
    fn servers_known(&self) -> bool;
    /// The output roots that need no server scan: the defaults and the custom roots the rc files name.
    fn config_output_roots(&self) -> Vec<PathBuf>;
    fn output_roots(&self, servers: &[Server]) -> Vec<PathBuf>;
    /// Runs `bazel shutdown` in workspace and reports whether it ran and succeeded.
    fn bazel_shutdown(&self, workspace: &Path) -> bool;
    /// Whether no Bazel client holds the server's output-base lock; `None` when unknown.
    fn server_idle(&self, server: &Server) -> Option<bool>;
    /// When the server last ran a command, in epoch seconds; `None` when unknown.
    fn server_last_used(&self, server: &Server) -> Option<u64>;
    /// Resident plus swapped bytes the server holds.
    fn server_footprint(&self, server: &Server) -> u64;
    /// Stops this one server without waiting for a busy output base; true once its pid is gone.
    fn shutdown_server(&self, server: &Server) -> bool;
    fn disk_bytes(&self, path: &Path) -> u64;
    fn disk_cache(&self) -> Option<PathBuf>;
    /// The slot count above which `add` deletes free slots whose commits are all pushed.
    fn max_slots(&self) -> usize {
        DEFAULT_MAX_SLOTS
    }
    /// Where nested-Bazel tests keep their slots, if this host has such a root.
    fn nested_root(&self) -> Option<PathBuf>;
    fn interrupted(&self) -> Option<i32>;
    /// Deletes a directory tree, including read-only entries; true when it is gone afterwards.
    #[must_use]
    fn remove_tree(&self, path: &Path) -> bool;
}

/// Compares two paths the way Bazel prints them.
pub fn same_path(left: &Path, right: &Path) -> bool {
    normalize(&resolved(left)) == normalize(&resolved(right))
}

/// Resolves symlinks in the deepest existing ancestor and keeps the missing tail.
pub(crate) fn resolved(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(real) = std::fs::canonicalize(ancestor) {
            let tail = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return if tail.as_os_str().is_empty() {
                real
            } else {
                real.join(tail)
            };
        }
    }
    path.to_path_buf()
}

fn normalize(path: &Path) -> String {
    let text = dx_path::posix(path);
    let text = if let Some(rest) = text.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else {
        text.strip_prefix("//?/").unwrap_or(&text).to_owned()
    };
    let text = text.trim_end_matches('/').to_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn flag_value(args: &str, flag: &str) -> Option<PathBuf> {
    if cfg!(windows) {
        windows_flag_value(args, flag)
    } else {
        ps_flag_value(args, flag)
    }
}

/// Reads a flag from a Windows command line, where quoted arguments may hold spaces.
fn windows_flag_value(line: &str, flag: &str) -> Option<PathBuf> {
    let key = format!("{flag}=");
    let value = windows_args(line)
        .into_iter()
        .find_map(|arg| arg.strip_prefix(&key).map(str::to_owned))?;
    non_empty_path(&value)
}

/// Reads a flag from `ps` output, which has no quoting. A value ends at the next ` --`.
fn ps_flag_value(args: &str, flag: &str) -> Option<PathBuf> {
    let start = args.find(&format!(" {flag}="))? + flag.len() + 2;
    let rest = &args[start..];
    let end = rest.find(" --").unwrap_or(rest.len());
    non_empty_path(&rest[..end])
}

fn non_empty_path(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

/// Splits a Windows command line into arguments with the C runtime quoting rules.
fn windows_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut arg = String::new();
    let mut in_arg = false;
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut count = 1;
                while chars.next_if_eq(&'\\').is_some() {
                    count += 1;
                }
                in_arg = true;
                if chars.next_if_eq(&'"').is_some() {
                    arg.extend(std::iter::repeat_n('\\', count / 2));
                    if count % 2 == 1 {
                        arg.push('"');
                    } else {
                        quoted = !quoted;
                    }
                } else {
                    arg.extend(std::iter::repeat_n('\\', count));
                }
            }
            '"' => {
                quoted = !quoted;
                in_arg = true;
            }
            ' ' | '\t' if !quoted => {
                if in_arg {
                    args.push(std::mem::take(&mut arg));
                    in_arg = false;
                }
            }
            _ => {
                arg.push(c);
                in_arg = true;
            }
        }
    }
    if in_arg {
        args.push(arg);
    }
    args
}

/// Builds a server from one process, or `None` when it is not a Bazel server.
pub(crate) fn server_from(pid: u32, rss_bytes: u64, args: &str) -> Option<Server> {
    let args = format!(" {}", args.trim_start());
    if !args.contains("A-server.jar") {
        return None;
    }
    Some(Server {
        pid,
        rss_bytes,
        workspace: flag_value(&args, "--workspace_directory"),
        output_base: flag_value(&args, "--output_base"),
        output_user_root: flag_value(&args, "--output_user_root"),
    })
}

/// Reads Bazel servers from `pid rss-kib args` lines.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn parse_servers(text: &str) -> Vec<Server> {
    let mut servers = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let mut parts = line.splitn(3, char::is_whitespace);
        let (Some(pid), Some(rss), Some(args)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(pid), Ok(rss)) = (pid.parse::<u32>(), rss.parse::<u64>()) else {
            continue;
        };
        servers.extend(server_from(pid, rss * 1024, args));
    }
    servers
}

/// Finds the output bases under each root that record a workspace path.
pub(crate) fn scan_output_bases(roots: &[PathBuf]) -> Vec<OutputBase> {
    let mut bases: Vec<OutputBase> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let Ok(text) = std::fs::read_to_string(dir.join("DO_NOT_BUILD_HERE")) else {
                continue;
            };
            let workspace = text.trim();
            if workspace.is_empty() || bases.iter().any(|base| base.dir == dir) {
                continue;
            }
            bases.push(OutputBase {
                dir,
                workspace: PathBuf::from(workspace),
            });
        }
    }
    bases.sort_by(|left, right| left.dir.cmp(&right.dir));
    bases
}

/// Whether any output base under `roots` was made for `workspace`.
pub fn has_output_base(roots: &[PathBuf], workspace: &Path) -> bool {
    scan_output_bases(roots)
        .iter()
        .any(|base| same_path(&base.workspace, workspace))
}

/// One entry of Bazel's repository cache download store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub dir: PathBuf,
    pub cache: PathBuf,
    pub used: u64,
    pub bytes: u64,
}

/// Where Bazel keeps downloads under an output root when `--repository_cache` is not set.
pub(crate) fn download_store(root: &Path) -> PathBuf {
    root.join("cache")
        .join("repos")
        .join("v1")
        .join("content_addressable")
}

/// Newest file modified time in a download entry, in epoch seconds.
pub(crate) fn download_used(dir: &Path) -> Option<u64> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut newest = None;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        newest = newest.max(Some(seconds(&meta)));
    }
    newest
}

fn seconds(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Finds the download entries in each root's default repository cache, `<algo>/<key>/` under `content_addressable`.
pub(crate) fn scan_downloads(roots: &[PathBuf], stop: &dyn Fn() -> Option<i32>) -> Vec<Download> {
    let mut downloads: Vec<Download> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        let cache = download_store(root);
        if seen.contains(&cache) {
            continue;
        }
        seen.push(cache.clone());
        let Ok(algos) = std::fs::read_dir(&cache) else {
            continue;
        };
        for algo in algos.flatten() {
            if !algo.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let Ok(keys) = std::fs::read_dir(algo.path()) else {
                continue;
            };
            for key in keys.flatten() {
                if stop().is_some() {
                    return downloads;
                }
                if !key.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let dir = key.path();
                let Some(used) = download_used(&dir) else {
                    continue;
                };
                downloads.push(Download {
                    bytes: walk_bytes(&dir, stop),
                    dir,
                    cache: cache.clone(),
                    used,
                });
            }
        }
    }
    downloads.sort_by(|left, right| left.dir.cmp(&right.dir));
    downloads.dedup_by(|left, right| left.dir == right.dir);
    downloads
}

fn walk_bytes(path: &Path, stop: &dyn Fn() -> Option<i32>) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        if stop().is_some() {
            break;
        }
        total += walk_bytes(&entry.path(), stop);
    }
    total
}

/// The `bazel shutdown` of a workspace's server; it fails at once if a client holds the lock. The server must
/// outlive a killed client, so descendants stay detachable.
fn shutdown_spec(bazel: &Path, opts: &[&str], workspace: &Path) -> SpawnSpec {
    let mut argv: Vec<std::ffi::OsString> = vec![bazel.as_os_str().to_owned()];
    argv.extend(opts.iter().map(std::ffi::OsString::from));
    argv.push("--noblock_for_lock".into());
    argv.push("shutdown".into());
    SpawnSpec {
        argv,
        cwd: workspace.to_path_buf(),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        capture: CapturePolicy { max_bytes: 1 << 16 },
        timeout: SHUTDOWN_TIMEOUT,
        descendants: Descendants::Detachable,
    }
}

fn du_bytes(path: &Path, stop: &dyn Fn() -> Option<i32>) -> Option<u64> {
    let spec = SpawnSpec {
        argv: vec!["du".into(), "-sk".into(), path.as_os_str().to_owned()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        capture: CapturePolicy { max_bytes: 1 << 20 },
        timeout: Duration::from_secs(3600),
        descendants: Descendants::Owned,
    };
    let ChildOutcome::Finished { stdout, .. } =
        run_within_until(&spec, CleanupBudget::STANDARD, stop).ok()?
    else {
        return None;
    };
    String::from_utf8_lossy(&stdout)
        .split_whitespace()
        .next()
        .and_then(|first| first.parse::<u64>().ok())
        .map(|kib| kib * 1024)
}

/// Swapped-out bytes from a `/proc/<pid>/status` text.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_vm_swap(status: &str) -> Option<u64> {
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("VmSwap:"))?;
    let kib = line.split_whitespace().next()?.parse::<u64>().ok()?;
    Some(kib * 1024)
}

/// The command line that stops exactly `server`, failing at once if a client holds its lock.
pub(crate) fn shutdown_argv(
    bazel: &Path,
    startup_opts: &[&str],
    server: &Server,
) -> Option<Vec<std::ffi::OsString>> {
    let base = server.output_base.as_deref()?;
    let root = server.output_user_root.as_deref()?;
    let mut argv: Vec<std::ffi::OsString> = vec![bazel.as_os_str().to_owned()];
    argv.extend(startup_opts.iter().map(std::ffi::OsString::from));
    let mut flag = |name: &str, value: &Path| {
        let mut arg = std::ffi::OsString::from(format!("--{name}="));
        arg.push(value.as_os_str());
        argv.push(arg);
    };
    flag("output_user_root", root);
    flag("output_base", base);
    argv.push("--noblock_for_lock".into());
    argv.push("shutdown".into());
    Some(argv)
}

/// Whether another process holds a write-conflicting fcntl lock on `file`; tests without taking it.
///
/// Bazel locks the output base with fcntl (OFD, else POSIX), which `flock` probes do not see on Linux.
#[cfg(unix)]
pub(crate) fn lock_busy(file: &std::fs::File) -> Option<bool> {
    use std::os::fd::AsRawFd;
    // SAFETY: a zeroed `flock` is a valid value; every field the call reads is set below.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock.l_start = 0;
    lock.l_len = 0;
    // SAFETY: the fd is open for the call and `lock` is a valid, exclusively borrowed `flock`.
    let status = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) };
    if status != 0 {
        return None;
    }
    Some(lock.l_type != libc::F_UNLCK as libc::c_short)
}

/// Whether the output base's lock file is free; `None` when it is missing or owned by another user.
#[cfg(unix)]
fn output_base_idle(base: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    let file = std::fs::File::open(base.join("lock")).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || Some(meta.uid()) != current_uid() {
        return None;
    }
    lock_busy(&file).map(|busy| !busy)
}

#[cfg(not(unix))]
fn output_base_idle(_base: &Path) -> Option<bool> {
    None
}

/// The newer modified time of the output base's `lock` and `command.log`.
fn output_base_used(base: &Path) -> Option<u64> {
    ["lock", "command.log"]
        .iter()
        .filter_map(|name| std::fs::metadata(base.join(name)).ok())
        .map(|meta| seconds(&meta))
        .max()
}

#[cfg(target_os = "linux")]
fn swap_bytes(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|text| parse_vm_swap(&text))
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn swap_bytes(_pid: u32) -> u64 {
    0
}

/// How long one targeted `bazel shutdown` may run.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a stopped server may take to exit after its client returns.
const EXIT_WAIT: Duration = Duration::from_secs(5);

/// The machine this process runs on.
pub struct SystemHost;

#[cfg(unix)]
fn list_servers() -> Option<String> {
    list_servers_with("ps")
}

#[cfg(unix)]
fn list_servers_with(ps: &str) -> Option<String> {
    dx_process::probe(ps, ["-eo", "pid=,rss=,args="])
}

fn home(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env_get("HOME")
        .or_else(|| env_get("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn user_name(env_get: &dyn Fn(&str) -> Option<String>) -> String {
    env_get("USER")
        .or_else(|| env_get("USERNAME"))
        .unwrap_or_default()
}

/// Where Bazel keeps output bases when nothing moves them.
pub fn default_output_roots(env_get: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let user = user_name(env_get);
    let mut roots = Vec::new();
    if let Some(cache) = dx_process::user_cache_dir(env_get) {
        roots.push(cache.join("bazel").join(format!("_bazel_{user}")));
    }
    if cfg!(windows)
        && let Some(home) = home(env_get)
    {
        roots.push(home.join(format!("_bazel_{user}")));
    }
    roots.push(PathBuf::from(format!("/private/var/tmp/_bazel_{user}")));
    roots
}

/// Custom `--output_user_root` values that `startup` lines in the rc files `files` name,
/// following `import` and `try-import` lines.
pub fn rc_output_roots(files: Vec<PathBuf>, workspace: &Path) -> Vec<PathBuf> {
    let mut pending = files;
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut roots = Vec::new();
    while let Some(file) = pending.pop() {
        if seen.contains(&file) || seen.len() > 64 {
            continue;
        }
        seen.push(file.clone());
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let tokens = dx_process::rc::tokenize(line);
            let mut words = tokens.iter().map(String::as_str);
            match words.next() {
                Some("startup") => {
                    while let Some(token) = words.next() {
                        if token.starts_with('#') {
                            break;
                        }
                        let root = match token {
                            "--output_user_root" => words.next(),
                            _ => token.strip_prefix("--output_user_root="),
                        };
                        if let Some(root) = root.map(|root| workspace.join(root))
                            && !roots.contains(&root)
                        {
                            roots.push(root);
                        }
                    }
                }
                Some("import" | "try-import") => {
                    let Some(path) = words.next() else { continue };
                    pending.push(match path.strip_prefix("%workspace%") {
                        Some(rest) => workspace.join(rest.trim_start_matches(['/', '\\'])),
                        None => PathBuf::from(path),
                    });
                }
                _ => {}
            }
        }
    }
    roots
}

/// Whether a process works in `dir` or below it, read from `/proc/<pid>/cwd`. Git processes are
/// skipped: dx runs them in the slot for its own probes. Processes this user cannot read are not seen.
#[cfg(target_os = "linux")]
fn process_in(dir: &Path) -> bool {
    let dir = resolved(dir);
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
            && std::fs::read_link(path.join("cwd")).is_ok_and(|cwd| cwd.starts_with(&dir))
            && std::fs::read_to_string(path.join("comm")).is_ok_and(|comm| comm.trim() != "git")
    })
}

#[cfg(not(target_os = "linux"))]
fn process_in(_dir: &Path) -> bool {
    false
}

fn find_bazel() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["bazel", "bazelisk", "bazel.exe", "bazelisk.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

impl Host for SystemHost {
    fn remove_tree(&self, path: &Path) -> bool {
        dx_process::remove_tree(path)
    }

    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0)
    }

    fn process_alive(&self, pid: u32) -> bool {
        dx_process::procs::pid_alive(pid)
    }

    fn process_start(&self, pid: u32) -> Option<String> {
        dx_process::procs::start_time(pid)
    }

    fn boot_id(&self) -> Option<String> {
        dx_process::procs::boot_id()
    }

    fn process_in(&self, dir: &Path) -> bool {
        process_in(dir)
    }

    fn disk_space(&self, path: &Path) -> Option<DiskSpace> {
        dx_process::disk_space(path)
    }

    fn servers(&self) -> Vec<Server> {
        #[cfg(unix)]
        {
            parse_servers(&list_servers().unwrap_or_default())
        }
        #[cfg(windows)]
        {
            dx_process::procs::command_lines()
                .into_iter()
                .filter_map(|info| server_from(info.pid, info.rss_bytes, &info.command_line))
                .collect()
        }
    }

    fn servers_known(&self) -> bool {
        #[cfg(unix)]
        {
            list_servers().is_some()
        }
        #[cfg(windows)]
        {
            true
        }
    }

    fn config_output_roots(&self) -> Vec<PathBuf> {
        let mut roots = default_output_roots(&|name| std::env::var(name).ok());
        if let Ok(workspace) = std::env::current_dir() {
            let mut files = vec![workspace.join(".bazelrc")];
            if let Some(home) = home(&|name| std::env::var(name).ok()) {
                files.push(home.join(".bazelrc"));
            }
            for root in rc_output_roots(files, &workspace) {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        roots
    }

    fn output_roots(&self, servers: &[Server]) -> Vec<PathBuf> {
        let mut roots = self.config_output_roots();
        for server in servers.iter().cloned() {
            if let Some(root) = server.output_user_root
                && !roots.contains(&root)
            {
                roots.push(root);
            }
        }
        roots
    }

    fn bazel_shutdown(&self, workspace: &Path) -> bool {
        let Some(bazel) = find_bazel() else {
            return false;
        };
        let spec = shutdown_spec(&bazel, dx_process::current().startup_opts(), workspace);
        run_within_until(&spec, CleanupBudget::STANDARD, &dx_process::interrupted)
            .is_ok_and(|outcome| {
                matches!(outcome, ChildOutcome::Finished { exit, .. } if exit.code() == Some(0))
            })
    }

    fn server_idle(&self, server: &Server) -> Option<bool> {
        output_base_idle(server.output_base.as_deref()?)
    }

    fn server_last_used(&self, server: &Server) -> Option<u64> {
        output_base_used(server.output_base.as_deref()?)
    }

    fn server_footprint(&self, server: &Server) -> u64 {
        server.rss_bytes.saturating_add(swap_bytes(server.pid))
    }

    fn shutdown_server(&self, server: &Server) -> bool {
        let Some(bazel) = find_bazel() else {
            return false;
        };
        let Some(argv) = shutdown_argv(&bazel, dx_process::current().startup_opts(), server) else {
            return false;
        };
        let cwd = server
            .workspace
            .clone()
            .filter(|dir| dir.is_dir())
            .or_else(|| server.output_user_root.clone())
            .unwrap_or_else(|| PathBuf::from("/"));
        let spec = SpawnSpec {
            argv,
            cwd,
            env: EnvPolicy::Inherited { extra: Vec::new() },
            capture: CapturePolicy { max_bytes: 1 << 16 },
            timeout: SHUTDOWN_TIMEOUT,
            descendants: Descendants::Detachable,
        };
        let stop: &dyn Fn() -> Option<i32> = &dx_process::interrupted;
        let _ = run_within_until(&spec, CleanupBudget::STANDARD, stop);
        let deadline = std::time::Instant::now() + EXIT_WAIT;
        while self.process_alive(server.pid) {
            if std::time::Instant::now() >= deadline || stop().is_some() {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        true
    }

    fn disk_bytes(&self, path: &Path) -> u64 {
        let stop: &dyn Fn() -> Option<i32> = &dx_process::interrupted;
        if cfg!(unix)
            && let Some(bytes) = du_bytes(path, stop)
        {
            return bytes;
        }
        if stop().is_some() {
            return 0;
        }
        walk_bytes(path, stop)
    }

    fn disk_cache(&self) -> Option<PathBuf> {
        dx_process::current().bazel_disk_cache.clone()
    }

    fn max_slots(&self) -> usize {
        std::env::var(MAX_SLOTS_ENV)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(DEFAULT_MAX_SLOTS)
    }

    fn nested_root(&self) -> Option<PathBuf> {
        let env = std::env::var_os(dx_path::nested_bazel::ROOT_ENV);
        let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
        Some(dx_path::nested_bazel::root(
            env.as_deref(),
            current_uid(),
            profile.as_deref(),
        ))
    }

    fn interrupted(&self) -> Option<i32> {
        dx_process::interrupted()
    }
}

#[cfg(unix)]
fn current_uid() -> Option<u32> {
    Some(unsafe { libc::getuid() })
}

#[cfg(not(unix))]
fn current_uid() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn default_output_roots_follow_the_user_cache_dir() {
        let roots = |pairs: &'static [(&'static str, &'static str)]| {
            default_output_roots(&|name| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            })
        };
        let paths = |items: &[&str]| items.iter().map(PathBuf::from).collect::<Vec<_>>();
        let home_root: &[&str] = if cfg!(windows) { &["/h/_bazel_u"] } else { &[] };
        let with_home = |mut head: Vec<&'static str>| {
            head.extend_from_slice(home_root);
            head.push("/private/var/tmp/_bazel_u");
            paths(&head)
        };
        assert_eq!(
            roots(&[("USER", "u"), ("XDG_CACHE_HOME", "/x"), ("HOME", "/h")]),
            with_home(vec!["/x/bazel/_bazel_u"])
        );
        assert_eq!(
            roots(&[("USER", "u"), ("HOME", "/h")])[0],
            PathBuf::from("/h/.cache/bazel/_bazel_u")
        );
        assert_eq!(
            roots(&[("USER", "u")]),
            paths(&["/private/var/tmp/_bazel_u"])
        );
    }

    #[test]
    fn server_from_reads_a_windows_command_line() {
        let line = r"C:\jdk\bin\java.exe -jar C:\i\A-server.jar --output_base=C:\x --workspace_directory=C:\w";
        let server = server_from(9, 5, line).expect("server");
        assert_eq!(server.pid, 9);
        assert_eq!(server.rss_bytes, 5);
        assert_eq!(server.workspace, Some(PathBuf::from(r"C:\w")));
        assert_eq!(server.output_base, Some(PathBuf::from(r"C:\x")));
        assert!(server_from(9, 5, r"C:\other.exe").is_none());
    }

    #[test]
    fn windows_command_lines_keep_quoted_paths_with_spaces() {
        let line = r#"C:\jdk\bin\java.exe -jar C:\i\A-server.jar "--output_base=C:\Users\John Smith\base" --workspace_directory="C:\my repo\w" --output_user_root=C:\root"#;
        assert_eq!(
            windows_flag_value(line, "--output_base"),
            Some(PathBuf::from(r"C:\Users\John Smith\base"))
        );
        assert_eq!(
            windows_flag_value(line, "--workspace_directory"),
            Some(PathBuf::from(r"C:\my repo\w"))
        );
        assert_eq!(
            windows_flag_value(line, "--output_user_root"),
            Some(PathBuf::from(r"C:\root"))
        );
        assert_eq!(windows_flag_value(line, "--missing"), None);
    }

    #[test]
    fn windows_args_follow_the_c_runtime_quoting_rules() {
        assert_eq!(
            windows_args(r#""C:\Program Files\java.exe" -a "b c" d\"e "f\\" g\\\"h"#),
            vec![
                r"C:\Program Files\java.exe",
                "-a",
                "b c",
                "d\"e",
                r"f\",
                "g\\\"h",
            ]
        );
    }

    #[test]
    fn servers_are_read_from_ps_lines_with_paths_that_hold_spaces() {
        let text = "  10 2048 /usr/bin/other --workspace_directory=/x\n\
 4242 1024 bazel(app) -Xmx4g -jar /i/A-server.jar --max_idle_secs=300 --output_user_root=/home/u/.cache/bazel/_bazel_u --output_base=/home/u/.cache/bazel/_bazel_u/abc --workspace_directory=/home/u/my repo/slot --default_system_javabase=/jdk\n\
 77 notanumber bazel(app) A-server.jar\n";
        let servers = parse_servers(text);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].pid, 4242);
        assert_eq!(servers[0].rss_bytes, 1024 * 1024);
        assert_eq!(
            servers[0].workspace,
            Some(PathBuf::from("/home/u/my repo/slot"))
        );
        assert_eq!(
            servers[0].output_base,
            Some(PathBuf::from("/home/u/.cache/bazel/_bazel_u/abc"))
        );
        assert_eq!(
            servers[0].output_user_root,
            Some(PathBuf::from("/home/u/.cache/bazel/_bazel_u"))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_working_below_a_directory_is_found() {
        let dir = tempfile::tempdir().expect("dir");
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).expect("sub");
        assert!(!process_in(dir.path()));
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .current_dir(&sub)
            .spawn()
            .expect("spawn");
        let found = process_in(dir.path());
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert!(found);
        assert!(!process_in(dir.path()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_git_process_working_in_a_directory_is_ignored() {
        let dir = tempfile::tempdir().expect("dir");
        let bin = tempfile::tempdir().expect("bin");
        let git = bin.path().join("git");
        std::os::unix::fs::symlink("/bin/sleep", &git).expect("link");
        let mut child = std::process::Command::new(&git)
            .arg("30")
            .current_dir(dir.path())
            .spawn()
            .expect("spawn");
        let found = process_in(dir.path());
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert!(!found);
    }

    #[test]
    fn paths_compare_across_separators_and_trailing_slashes() {
        assert!(same_path(Path::new("/a/b/"), Path::new("/a/b")));
        assert!(same_path(Path::new("C:\\a\\b"), Path::new("C:/a/b")));
        assert!(!same_path(Path::new("/a/b"), Path::new("/a/c")));
        assert!(same_path(Path::new("\\\\?\\C:\\a\\b"), Path::new("C:/a/b")));
        assert_eq!(
            normalize(Path::new("\\\\?\\UNC\\srv\\s\\x")),
            normalize(Path::new("//srv/s/x"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn paths_compare_through_symlinks_even_when_the_tail_is_gone() {
        let scratch = dx_test_scratch::scratch("dx-wt-links-");
        let real = scratch.path().join("real");
        let link = scratch.path().join("link");
        std::fs::create_dir_all(real.join("x")).expect("real");
        std::os::unix::fs::symlink(&real, &link).expect("link");
        assert!(same_path(&link.join("x"), &real.join("x")));
        std::fs::remove_dir(real.join("x")).expect("remove");
        assert!(same_path(&link.join("x"), &real.join("x")));
        assert!(same_path(&link.join("x").join("y"), &real.join("x/y")));
        assert!(!same_path(&link.join("x"), &real.join("y")));
        assert!(same_path(Path::new("/gone/a/b"), Path::new("/gone/a/b/")));
        assert!(!same_path(Path::new("/gone/a/b"), Path::new("/gone/a/c")));
    }

    #[test]
    fn output_bases_are_found_by_their_workspace_marker() {
        let scratch = dx_test_scratch::scratch("dx-wt-bases-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(root.join("install")).expect("install");
        std::fs::create_dir_all(root.join("aaa")).expect("base");
        std::fs::write(root.join("aaa").join("DO_NOT_BUILD_HERE"), "/gone/ws\n").expect("marker");
        let bases = scan_output_bases(&[root.clone(), root.clone(), scratch.path().join("none")]);
        assert_eq!(
            bases,
            vec![OutputBase {
                dir: root.join("aaa"),
                workspace: PathBuf::from("/gone/ws"),
            }]
        );
    }

    #[test]
    fn walked_size_counts_files_and_skips_symlink_targets() {
        let scratch = dx_test_scratch::scratch("dx-wt-size-");
        let dir = scratch.path().join("d");
        std::fs::create_dir_all(dir.join("sub")).expect("dirs");
        std::fs::write(dir.join("a"), vec![0u8; 100]).expect("a");
        std::fs::write(dir.join("sub").join("b"), vec![0u8; 50]).expect("b");
        assert!(walk_bytes(&dir, &|| None) >= 150);
        assert_eq!(walk_bytes(&dir.join("missing"), &|| None), 0);
    }

    #[test]
    fn walk_bytes_returns_early_when_stopped() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-walk-");
        let dir = scratch.path().join("d");
        for sub in ["one", "two"] {
            std::fs::create_dir_all(dir.join(sub)).expect("dir");
            std::fs::write(dir.join(sub).join("f"), vec![0u8; 100]).expect("file");
        }
        let calls = std::cell::Cell::new(0);
        let stop = || {
            calls.set(calls.get() + 1);
            (calls.get() > 1).then_some(15)
        };
        let full = walk_bytes(&dir, &|| None);
        assert_eq!(full, 200);
        assert!(walk_bytes(&dir, &stop) < full);
    }

    fn aged(path: &Path, secs: u64) {
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open");
        file.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(secs))
            .expect("set time");
    }

    fn download(root: &Path, key: &str, files: &[(&str, u64)]) -> PathBuf {
        let dir = download_store(root).join("sha256").join(key);
        std::fs::create_dir_all(&dir).expect("entry");
        for (name, secs) in files {
            std::fs::write(dir.join(name), vec![0u8; 10]).expect("file");
            aged(&dir.join(name), *secs);
        }
        dir
    }

    #[test]
    fn downloads_are_found_with_their_newest_file_time() {
        let scratch = dx_test_scratch::scratch("dx-wt-downloads-");
        let root = scratch.path().join("root");
        let old = download(&root, "aaa", &[("file", 100)]);
        let reading = download(&root, "bbb", &[("file", 100), ("id-x", 500)]);
        let writing = download(&root, "ccc", &[("file", 100), ("tmp-1", 900)]);
        std::fs::create_dir_all(download_store(&root).join("sha256").join("empty")).expect("dir");
        let found = scan_downloads(
            &[root.clone(), root.clone(), scratch.path().join("none")],
            &|| None,
        );
        let cache = download_store(&root);
        assert_eq!(
            found,
            vec![
                Download {
                    dir: old,
                    cache: cache.clone(),
                    used: 100,
                    bytes: 10,
                },
                Download {
                    dir: reading.clone(),
                    cache: cache.clone(),
                    used: 500,
                    bytes: 20,
                },
                Download {
                    dir: writing,
                    cache,
                    used: 900,
                    bytes: 20,
                },
            ]
        );
        aged(&reading.join("file"), 700);
        assert_eq!(download_used(&reading), Some(700));
        assert_eq!(download_used(&reading.join("gone")), None);
    }

    #[test]
    fn a_root_without_a_repository_cache_has_no_downloads() {
        let scratch = dx_test_scratch::scratch("dx-wt-no-downloads-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(root.join("cache").join("repos").join("v1").join("contents"))
            .expect("contents");
        assert!(scan_downloads(&[root], &|| None).is_empty());
    }

    #[test]
    fn scan_downloads_stops_when_the_stop_probe_fires() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-downloads-");
        let root = scratch.path().join("root");
        download(&root, "aaa", &[("file", 100)]);
        download(&root, "bbb", &[("file", 100)]);
        assert_eq!(
            scan_downloads(std::slice::from_ref(&root), &|| None).len(),
            2
        );
        assert!(scan_downloads(&[root], &|| Some(15)).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn du_stops_when_the_stop_probe_fires() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-du-");
        assert_eq!(du_bytes(scratch.path(), &|| Some(15)), None);
    }

    #[cfg(unix)]
    #[test]
    fn du_measures_a_tree_when_not_stopped() {
        let scratch = dx_test_scratch::scratch("dx-wt-du-");
        std::fs::write(scratch.path().join("f"), vec![1u8; 64 * 1024]).expect("file");
        assert!(du_bytes(scratch.path(), &|| None).is_some_and(|bytes| bytes > 0));
    }

    #[test]
    fn vm_swap_is_read_in_bytes() {
        let status = "Name:\tjava\nVmRSS:\t  2048 kB\nVmSwap:\t   512 kB\nThreads:\t9\n";
        assert_eq!(parse_vm_swap(status), Some(512 * 1024));
        assert_eq!(parse_vm_swap("VmSwap:\t0 kB\n"), Some(0));
        assert_eq!(parse_vm_swap("Name:\tjava\n"), None);
    }

    #[test]
    fn the_workspace_shutdown_never_waits_for_a_lock_and_keeps_the_server_detachable() {
        let spec = shutdown_spec(Path::new("/bin/bazel"), &["--nohome_rc"], Path::new("/w"));
        assert_eq!(
            spec.argv,
            [
                "/bin/bazel",
                "--nohome_rc",
                "--noblock_for_lock",
                "shutdown"
            ]
        );
        assert_eq!(spec.timeout, SHUTDOWN_TIMEOUT);
        assert_eq!(spec.cwd, Path::new("/w"));
        assert!(matches!(spec.descendants, Descendants::Detachable));
    }

    #[test]
    fn shutdown_targets_the_servers_own_base_and_never_waits_for_its_lock() {
        let server = Server {
            pid: 7,
            rss_bytes: 0,
            workspace: Some(PathBuf::from("/w")),
            output_base: Some(PathBuf::from("/r/abc")),
            output_user_root: Some(PathBuf::from("/r")),
        };
        let argv = shutdown_argv(Path::new("/bin/bazel"), &["--nohome_rc"], &server).expect("argv");
        let argv: Vec<String> = argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            argv,
            [
                "/bin/bazel",
                "--nohome_rc",
                "--output_user_root=/r",
                "--output_base=/r/abc",
                "--noblock_for_lock",
                "shutdown",
            ]
        );
        let no_base = Server {
            output_base: None,
            ..server.clone()
        };
        assert!(shutdown_argv(Path::new("/bin/bazel"), &[], &no_base).is_none());
        let no_root = Server {
            output_user_root: None,
            ..server
        };
        assert!(shutdown_argv(Path::new("/bin/bazel"), &[], &no_root).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_idle_probe_sees_fcntl_locks_and_ignores_flock() {
        use std::os::fd::AsRawFd;
        let scratch = dx_test_scratch::scratch("dx-wt-idle-");
        let base = scratch.path().join("base");
        std::fs::create_dir_all(&base).expect("base");
        let lock = base.join("lock");
        std::fs::write(&lock, "").expect("lock");
        assert_eq!(output_base_idle(&base), Some(true));

        let flocked = std::fs::File::open(&lock).expect("open");
        flocked.try_lock().expect("flock");
        assert_eq!(output_base_idle(&base), Some(true));
        drop(flocked);

        let held = std::fs::File::options()
            .read(true)
            .write(true)
            .open(&lock)
            .expect("open");
        // SAFETY: a zeroed `flock` is valid; the fields the call reads are set below.
        let mut request: libc::flock = unsafe { std::mem::zeroed() };
        request.l_type = libc::F_WRLCK as libc::c_short;
        request.l_whence = libc::SEEK_SET as libc::c_short;
        // SAFETY: the fd is open and `request` is a valid `flock`.
        let status = unsafe { libc::fcntl(held.as_raw_fd(), libc::F_OFD_SETLK, &mut request) };
        assert_eq!(status, 0, "{}", std::io::Error::last_os_error());
        assert_eq!(output_base_idle(&base), Some(false));
        drop(held);
        assert_eq!(output_base_idle(&base), Some(true));
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_lock_file_is_unknown_and_use_follows_the_newest_file() {
        let scratch = dx_test_scratch::scratch("dx-wt-used-");
        let base = scratch.path().join("base");
        std::fs::create_dir_all(&base).expect("base");
        assert_eq!(output_base_idle(&base), None);
        assert_eq!(output_base_used(&base), None);
        std::fs::write(base.join("lock"), "").expect("lock");
        std::fs::write(base.join("command.log"), "").expect("log");
        aged(&base.join("lock"), 100);
        aged(&base.join("command.log"), 300);
        assert_eq!(output_base_used(&base), Some(300));
        let server = Server {
            pid: 1,
            rss_bytes: 0,
            workspace: None,
            output_base: None,
            output_user_root: None,
        };
        assert_eq!(SystemHost.server_idle(&server), None);
        assert_eq!(SystemHost.server_last_used(&server), None);
    }

    #[test]
    fn this_process_is_alive_and_a_reaped_child_is_not() {
        let host = SystemHost;
        assert!(host.process_alive(std::process::id()));
        let mut child = if cfg!(windows) {
            Command::new("cmd").args(["/C", "exit 0"]).spawn()
        } else {
            Command::new("true").spawn()
        }
        .expect("spawn");
        let pid = child.id();
        child.wait().expect("wait");
        assert!(!host.process_alive(pid));
    }

    #[cfg(unix)]
    #[cfg(unix)]
    #[test]
    fn a_missing_ps_is_unknown_not_empty() {
        let listed = list_servers_with("dx-no-such-ps");
        assert_eq!(listed, None);
    }

    #[test]
    fn rc_output_roots_read_startup_lines_and_imports() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join(".bazelrc"),
            "build --foo\nstartup --output_user_root=/mnt/big/bazel # c\nimport %workspace%/extra.rc\n",
        )
        .expect("rc");
        std::fs::write(
            dir.path().join("extra.rc"),
            "startup --output_user_root=/mnt/other\n",
        )
        .expect("extra");
        let mut roots = rc_output_roots(vec![dir.path().join(".bazelrc")], dir.path());
        roots.sort();
        assert_eq!(
            roots,
            vec![PathBuf::from("/mnt/big/bazel"), PathBuf::from("/mnt/other")]
        );
    }

    #[test]
    fn rc_output_roots_join_relative_startup_roots_to_the_workspace() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join(".bazelrc"),
            "startup --output_user_root=cache/bazel\nstartup --output_user_root=\"cache/other\"\n",
        )
        .expect("rc");
        let roots = rc_output_roots(vec![dir.path().join(".bazelrc")], dir.path());
        assert_eq!(
            roots,
            vec![
                dir.path().join("cache/bazel"),
                dir.path().join("cache/other")
            ]
        );
    }

    #[test]
    fn rc_output_roots_keep_quoted_paths_with_spaces() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join(".bazelrc"),
            "startup --output_user_root=\"/tmp/my cache\"\nimport \"%workspace%/my rc\"\n",
        )
        .expect("rc");
        std::fs::write(
            dir.path().join("my rc"),
            "startup --output_user_root \"/tmp/other cache\" # note\n",
        )
        .expect("extra");
        let mut roots = rc_output_roots(vec![dir.path().join(".bazelrc")], dir.path());
        roots.sort();
        assert_eq!(
            roots,
            vec![
                PathBuf::from("/tmp/my cache"),
                PathBuf::from("/tmp/other cache")
            ]
        );
    }
}
