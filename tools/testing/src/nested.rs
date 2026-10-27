use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::time::{Duration, Instant};

use dx_path::nested_bazel;

use crate::Run;

pub const MAX_IDLE_SECS: u32 = 5;
pub const DEFAULT_HEAP: &str = "1536m";
pub const DEFAULT_SLOTS: usize = 3;
pub const SLOTS_ENV: &str = "DX_NESTED_BAZEL_SLOTS";
pub const PINNED_ENV: &str = "DX_NESTED_BAZEL_PINNED";
pub const CPUS: u32 = 2;
pub const STARTUP: &[&str] = &["--nohome_rc", "--nosystem_rc"];

const STRIPPED_PREFIXES: &[&str] = &["TEST_", "RUNFILES_"];
const STRIPPED: &[&str] = &[
    "BUILD_WORKING_DIRECTORY",
    "BUILD_WORKSPACE_DIRECTORY",
    "JAVA_RUNFILES",
    "PYTHON_RUNFILES",
    "XML_OUTPUT_FILE",
];

/// Settings for the nested Bazel server of one test binary.
#[derive(Clone, Debug)]
pub struct Config {
    pub base: PathBuf,
    pub slots: usize,
    pub bazel: PathBuf,
    pub heap: String,
    pub disk_cache: PathBuf,
    pub repository_cache: Option<PathBuf>,
    pub offline: bool,
}

impl Config {
    /// Reads the settings from the environment and the outer `~/.bazelrc`.
    pub fn from_env() -> Result<Config, String> {
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from);
        let rc = home
            .as_ref()
            .and_then(|home| std::fs::read_to_string(home.join(".bazelrc")).ok())
            .unwrap_or_default();
        Self::from_parts(
            &|name| std::env::var_os(name),
            &resolve_pinned,
            &rc,
            home.as_deref(),
        )
    }

    /// Builds the settings from an environment lookup, the outer bazelrc text and the home.
    fn from_parts(
        lookup: &dyn Fn(&str) -> Option<OsString>,
        resolve: &dyn Fn(&str) -> Result<PathBuf, String>,
        rc: &str,
        home: Option<&Path>,
    ) -> Result<Config, String> {
        let text = |name: &str| lookup(name).and_then(|v| v.to_str().map(str::to_owned));
        let base = match env_path(lookup, nested_bazel::ROOT_ENV) {
            Some(base) => base,
            None => default_base()?,
        };
        let slots = text(SLOTS_ENV)
            .and_then(|slots| slots.parse::<usize>().ok())
            .filter(|slots| *slots > 0)
            .unwrap_or(DEFAULT_SLOTS);
        let heap = text("DX_NESTED_BAZEL_HEAP")
            .filter(|heap| !heap.is_empty())
            .unwrap_or_else(|| DEFAULT_HEAP.to_owned());
        let path = lookup("PATH").unwrap_or_default();
        let chosen = env_path(lookup, "DX_NESTED_BAZEL");
        let pinned = match text(PINNED_ENV).filter(|key| !key.is_empty()) {
            Some(key) if chosen.is_none() => Some(resolve(&key)?),
            _ => None,
        };
        let bazel = find_bazel(chosen, pinned, &path, &base)?;
        let disk_cache = disk_cache(
            env_path(lookup, "DX_NESTED_BAZEL_DISK_CACHE"),
            rc,
            home,
            &base,
        );
        let repository_cache = env_path(lookup, "DX_NESTED_BAZEL_REPOSITORY_CACHE")
            .or_else(|| rc_path(rc, "repository_cache", home))
            .or_else(|| home.and_then(default_repository_cache));
        Ok(Config {
            base,
            slots,
            bazel,
            heap,
            disk_cache,
            repository_cache,
            offline: lookup("DX_NESTED_BAZEL_NETWORK").is_none_or(|v| v.is_empty()),
        })
    }
}

fn resolve_pinned(key: &str) -> Result<PathBuf, String> {
    let found = crate::rlocation(key)
        .map_err(|error| format!("{PINNED_ENV} {key} is not in the runfiles: {error}"))?;
    std::path::absolute(&found)
        .map_err(|error| format!("{PINNED_ENV} {}: {error}", found.display()))
}

fn env_path(lookup: &dyn Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Returns our effective uid, read from a file we create exclusively so a pre-made path cannot lie.
#[cfg(unix)]
fn current_uid() -> Result<u32, String> {
    use std::os::unix::fs::MetadataExt;
    let dir = crate::mkscratch("nested-bazel-uid")
        .map_err(|error| format!("cannot create a scratch directory: {error}"))?;
    let probe = dir.join("uid");
    let result = File::options()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|file| file.metadata())
        .map(|meta| meta.uid())
        .map_err(|error| {
            format!(
                "cannot probe the current uid at {}: {error}",
                probe.display()
            )
        });
    let _ = std::fs::remove_file(&probe);
    let _ = std::fs::remove_dir(&dir);
    result
}

#[cfg(unix)]
fn default_base() -> Result<PathBuf, String> {
    Ok(nested_bazel::root(None, Some(current_uid()?)))
}

#[cfg(not(unix))]
fn default_base() -> Result<PathBuf, String> {
    Ok(nested_bazel::root(None, None))
}

/// Returns the configured disk cache, or one cache shared by every slot under the base.
pub fn disk_cache(env: Option<PathBuf>, rc: &str, home: Option<&Path>, base: &Path) -> PathBuf {
    env.or_else(|| rc_path(rc, "disk_cache", home))
        .unwrap_or_else(|| base.join("disk-cache"))
}

/// Returns the last `--<flag>=<path>` value in a bazelrc, expanding `~/`.
pub fn rc_path(rc: &str, flag: &str, home: Option<&Path>) -> Option<PathBuf> {
    let prefix = format!("--{flag}=");
    let mut found = None;
    for line in rc.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        for token in line.split_whitespace() {
            let Some(value) = token.strip_prefix(prefix.as_str()) else {
                continue;
            };
            let path = match value.strip_prefix("~/") {
                Some(rest) => home.map(|home| home.join(rest)),
                None => Some(PathBuf::from(value)),
            };
            if let Some(path) = path.filter(|path| path.is_absolute()) {
                found = Some(path);
            }
        }
    }
    found
}

fn default_repository_cache(home: &Path) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = std::fs::read_dir(home.join(".cache/bazel"))
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("_bazel_"))
        .map(|entry| entry.path().join("cache/repos/v1"))
        .filter(|path| path.is_dir())
        .collect();
    roots.sort();
    roots.into_iter().next()
}

/// Returns the `DX_NESTED_BAZEL` override, else the pinned binary, else `bazel` or `bazelisk` on `path`.
pub fn find_bazel(
    chosen: Option<PathBuf>,
    pinned: Option<PathBuf>,
    path: &std::ffi::OsStr,
    base: &Path,
) -> Result<PathBuf, String> {
    if let Some(chosen) = chosen {
        if chosen.is_file() {
            return Ok(chosen);
        }
        return Err(format!(
            "DX_NESTED_BAZEL {} is not a file",
            chosen.display()
        ));
    }
    if let Some(pinned) = pinned {
        if pinned.is_file() {
            return Ok(pinned);
        }
        return Err(format!("{PINNED_ENV} {} is not a file", pinned.display()));
    }
    for name in ["bazel", "bazelisk"] {
        for dir in std::env::split_paths(path) {
            if dir.starts_with(base) {
                continue;
            }
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "no bazel or bazelisk: set DX_NESTED_BAZEL, add the pinned binary as {PINNED_ENV}, or put bazel on PATH"
    ))
}

/// Quotes one argument for a POSIX shell.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("cannot chmod {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(unix)]
fn make_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let mode = meta.permissions().mode();
    if mode & 0o700 != 0o700 {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o700));
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(Result::ok) {
            make_writable(&entry.path());
        }
    }
}

#[cfg(not(unix))]
fn make_writable(_path: &Path) {}

/// Empties `root`, keeping the directory itself.
pub fn reset(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("cannot list {}: {error}", root.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let is_dir = std::fs::symlink_metadata(&path)
            .map(|meta| meta.is_dir())
            .unwrap_or(false);
        let removed = if is_dir {
            make_writable(&path);
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Renders the `bazel` wrapper that pins every nested startup option.
pub fn wrapper_script(config: &Config, user_root: &Path, rc: &Path) -> String {
    let argv = [
        config.bazel.to_string_lossy().into_owned(),
        format!("--output_user_root={}", user_root.display()),
        format!("--max_idle_secs={MAX_IDLE_SECS}"),
        format!("--host_jvm_args=-Xmx{}", config.heap),
        format!("--bazelrc={}", rc.display()),
    ];
    let quoted: Vec<String> = argv.iter().map(|arg| shell_quote(arg)).collect();
    format!("#!/bin/sh\nexec {} \"$@\"\n", quoted.join(" "))
}

/// Renders the bazelrc every nested command reads last.
pub fn bazelrc(config: &Config, downloader: &Path) -> String {
    let mut text = format!("common --local_resources=cpu={CPUS}\n");
    if config.offline {
        text.push_str(&format!(
            "common --downloader_config={}\n",
            downloader.display()
        ));
    }
    text.push_str(&format!(
        "common --disk_cache={}\n",
        config.disk_cache.display()
    ));
    if let Some(cache) = &config.repository_cache {
        text.push_str(&format!("common --repository_cache={}\n", cache.display()));
    }
    text
}

#[cfg(unix)]
mod signal {
    pub const SIGTERM: i32 = 15;
    pub const SIGKILL: i32 = 9;

    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    pub fn send(pid: u32, sig: i32) -> bool {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        unsafe { kill(pid, sig) == 0 }
    }

    pub fn alive(pid: u32) -> bool {
        send(pid, 0)
    }
}

#[cfg(not(unix))]
mod signal {
    pub const SIGTERM: i32 = 15;
    pub const SIGKILL: i32 = 9;

    pub fn send(_pid: u32, _sig: i32) -> bool {
        false
    }

    pub fn alive(_pid: u32) -> bool {
        false
    }
}

/// Reports whether `pid` is a live process.
pub fn alive(pid: u32) -> bool {
    if !signal::alive(pid) {
        return false;
    }
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        Err(_) => true,
    }
}

fn command_line(pid: u32) -> Option<String> {
    if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) {
        return Some(String::from_utf8_lossy(&cmdline).into_owned());
    }
    if Path::new("/proc/self").exists() {
        return None;
    }
    let output = Command::new("ps")
        .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn owned_by(pid: u32, user_root: &Path) -> bool {
    command_line(pid).is_some_and(|line| line.contains(&*user_root.to_string_lossy()))
}

/// Lists the live Bazel servers under one output user root.
pub fn servers(user_root: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(user_root) else {
        return Vec::new();
    };
    let mut pids: Vec<u32> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            std::fs::read_to_string(entry.path().join("server/server.pid.txt")).ok()
        })
        .filter_map(|text| text.trim().parse::<u32>().ok())
        .filter(|pid| alive(*pid) && owned_by(*pid, user_root))
        .collect();
    pids.sort_unstable();
    pids
}

fn wait_gone(pids: &[u32], limit: Duration) -> Vec<u32> {
    let started = Instant::now();
    loop {
        let left: Vec<u32> = pids.iter().copied().filter(|pid| alive(*pid)).collect();
        if left.is_empty() || started.elapsed() >= limit {
            return left;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn all_pids() -> Vec<u32> {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        return entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
            .collect();
    }
    let Some(output) = Command::new("ps")
        .args(["-A", "-o", "pid="])
        .output()
        .ok()
        .filter(|output| output.status.success())
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

/// Lists the live Bazel servers whose workspace is under `workspaces`, whatever their output root.
pub fn servers_in_workspaces(workspaces: &Path) -> Vec<u32> {
    let mut prefixes = vec![workspaces.to_string_lossy().into_owned()];
    if let Ok(real) = std::fs::canonicalize(workspaces) {
        prefixes.push(real.to_string_lossy().into_owned());
    }
    let own = std::process::id();
    let mut pids: Vec<u32> = all_pids()
        .into_iter()
        .filter(|pid| *pid != own && alive(*pid))
        .filter(|pid| {
            command_line(*pid).is_some_and(|line| {
                line.split(['\0', ' ']).any(|arg| {
                    arg.strip_prefix("--workspace_directory=")
                        .is_some_and(|dir| {
                            prefixes.iter().any(|prefix| {
                                dir.strip_prefix(prefix.as_str())
                                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
                            })
                        })
                })
            })
        })
        .collect();
    pids.sort_unstable();
    pids
}

/// Stops every server under `user_root` and returns the ones that survive.
pub fn kill_servers(user_root: &Path) -> Vec<u32> {
    kill_pids(servers(user_root))
}

/// Stops every server whose workspace is under `workspaces` and returns the ones that survive.
pub fn kill_servers_in_workspaces(workspaces: &Path) -> Vec<u32> {
    kill_pids(servers_in_workspaces(workspaces))
}

fn kill_pids(mut pids: Vec<u32>) -> Vec<u32> {
    pids.sort_unstable();
    pids.dedup();
    if pids.is_empty() {
        return pids;
    }
    for pid in &pids {
        signal::send(*pid, signal::SIGTERM);
    }
    let left = wait_gone(&pids, Duration::from_secs(5));
    for pid in &left {
        signal::send(*pid, signal::SIGKILL);
    }
    wait_gone(&left, Duration::from_secs(5))
}

fn stripped(key: &str) -> bool {
    STRIPPED.contains(&key)
        || STRIPPED_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

/// One nested Bazel server: a locked slot, its output user root and its workspace.
pub struct Server {
    config: Config,
    home: PathBuf,
    workspace: Option<PathBuf>,
    _slot: File,
    reaper: Option<Child>,
}

impl Server {
    /// Takes the first free slot under the configured base, waiting while all are busy.
    pub fn acquire(config: Config) -> Result<Server, String> {
        create_private(&config.base)?;
        let mut waiting = false;
        loop {
            for index in 0..config.slots {
                let home = config.base.join(index.to_string());
                std::fs::create_dir_all(&home)
                    .map_err(|error| format!("cannot create {}: {error}", home.display()))?;
                let lock = home.join(nested_bazel::LOCK);
                let file = File::options()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(&lock)
                    .map_err(|error| format!("cannot open {}: {error}", lock.display()))?;
                match file.try_lock() {
                    Ok(()) => return Server::claim(config, home, file),
                    Err(std::fs::TryLockError::WouldBlock) => {}
                    Err(std::fs::TryLockError::Error(error)) => {
                        return Err(format!("cannot lock {}: {error}", lock.display()))
                    }
                }
            }
            if !waiting {
                eprintln!(
                    "waiting for one of {} nested Bazel slots under {}",
                    config.slots,
                    config.base.display()
                );
                waiting = true;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn claim(config: Config, home: PathBuf, slot: File) -> Result<Server, String> {
        let mut server = Server {
            config,
            home,
            workspace: None,
            _slot: slot,
            reaper: None,
        };
        kill_servers(&server.user_root());
        kill_servers_in_workspaces(&server.home.join(nested_bazel::WORKSPACES));
        server.reaper = spawn_reaper(std::process::id(), &server.home, &server._slot);
        let rc = server.home.join("bazelrc");
        let downloader = server.home.join("downloader.cfg");
        write(&downloader, DOWNLOADER)?;
        write(&rc, &bazelrc(&server.config, &downloader))?;
        let wrapper = server.wrapper();
        write(
            &wrapper,
            &wrapper_script(&server.config, &server.user_root(), &rc),
        )?;
        make_executable(&wrapper)?;
        Ok(server)
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn user_root(&self) -> PathBuf {
        self.home.join(nested_bazel::USER_ROOT)
    }

    pub fn wrapper(&self) -> PathBuf {
        self.home.join("bin").join("bazel")
    }

    /// Empties the workspace `name` and makes it current, stopping the server of any other one.
    pub fn enter(&mut self, name: &str) -> Result<PathBuf, String> {
        let root = self.home.join(nested_bazel::WORKSPACES).join(name);
        if self.workspace.as_ref() != Some(&root) {
            self.shutdown();
            self.workspace = Some(root.clone());
        }
        reset(&root)?;
        Ok(root)
    }

    /// Lists the live servers this slot owns.
    pub fn servers(&self) -> Vec<u32> {
        let mut pids = servers(&self.user_root());
        pids.extend(servers_in_workspaces(
            &self.home.join(nested_bazel::WORKSPACES),
        ));
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    /// Stops the server and returns any that survive.
    pub fn shutdown(&mut self) -> Vec<u32> {
        if let Some(root) = self.workspace.as_ref().filter(|root| root.is_dir()) {
            if !self.servers().is_empty() {
                let _ = self.bazel_command(root, &["shutdown"]).output();
            }
        }
        let strays = kill_servers_in_workspaces(&self.home.join(nested_bazel::WORKSPACES));
        let mut left = kill_servers(&self.user_root());
        left.extend(strays);
        left
    }

    fn apply_env(&self, command: &mut Command, envs: &[(&str, &str)]) {
        apply_env_at(&self.home, command, envs);
    }

    /// Builds a command for `bin` that reaches Bazel only through this server.
    pub fn command(&self, bin: &Path, envs: &[(&str, &str)]) -> Result<Command, String> {
        let mut command = crate::command_for(bin, &[])
            .map_err(|error| format!("cannot prepare {}: {error}", bin.display()))?;
        self.apply_env(&mut command, envs);
        Ok(command)
    }

    /// Builds a `bazel` command in `root` with the startup options dx uses.
    pub fn bazel_command(&self, root: &Path, args: &[&str]) -> Command {
        bazel_command_at(&self.home, root, args, &[])
    }
}

fn apply_env_at(home: &Path, command: &mut Command, envs: &[(&str, &str)]) {
    let explicit: Vec<OsString> = command
        .get_envs()
        .filter(|(_, value)| value.is_some())
        .map(|(key, _)| key.to_owned())
        .collect();
    for (key, _) in std::env::vars_os() {
        if stripped(&key.to_string_lossy()) && !explicit.contains(&key) {
            command.env_remove(&key);
        }
    }
    let mut path = None;
    for (key, value) in envs {
        if *key == "PATH" {
            path = Some(OsString::from(value));
        } else {
            command.env(key, value);
        }
    }
    let path = path
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let mut dirs = vec![home.join("bin")];
    dirs.extend(std::env::split_paths(&path));
    if let Ok(joined) = std::env::join_paths(dirs) {
        command.env("PATH", joined);
    }
}

fn bazel_command_at(home: &Path, root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(home.join("bin").join("bazel"));
    command.current_dir(root).args(STARTUP).args(args);
    apply_env_at(home, &mut command, envs);
    command
}

#[cfg(unix)]
const REAPER: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 1; done
for sig in TERM KILL; do
  pids=$(ps -eo pid=,args= | awk -v a=" --workspace_directory=$2/" -v b=" --workspace_directory=$3/" 'index($0, a) || index($0, b) { print $1 }')
  [ -z "$pids" ] && exit 0
  kill -$sig $pids 2>/dev/null
  sleep 2
done
"#;

/// Starts a detached watcher that sweeps the slot's workspaces once `owner` is gone, however it died.
#[cfg(unix)]
fn spawn_reaper(owner: u32, home: &Path, slot: &File) -> Option<Child> {
    use std::os::unix::process::CommandExt;
    let workspaces = home.join(nested_bazel::WORKSPACES);
    let real = std::fs::canonicalize(&workspaces).unwrap_or_else(|_| workspaces.clone());
    let lock = slot.try_clone().ok()?;
    Command::new("sh")
        .arg("-c")
        .arg(REAPER)
        .arg("dx-nested-reaper")
        .arg(owner.to_string())
        .arg(&workspaces)
        .arg(&real)
        .stdin(std::process::Stdio::from(lock))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
        .ok()
}

#[cfg(not(unix))]
fn spawn_reaper(_owner: u32, _home: &Path, _slot: &File) -> Option<Child> {
    None
}

impl Drop for Server {
    fn drop(&mut self) {
        let left = self.shutdown();
        if let Some(mut reaper) = self.reaper.take() {
            let _ = reaper.kill();
            let _ = reaper.wait();
        }
        if !left.is_empty() {
            eprintln!("nested Bazel servers survived shutdown: {left:?}");
        }
    }
}

/// Explains why `path` is not a private directory of `me`, or returns None when it is.
#[cfg(unix)]
fn private_dir_error(
    path: &Path,
    is_symlink: bool,
    is_dir: bool,
    owner: u32,
    mode: u32,
    me: u32,
) -> Option<String> {
    let fix = format!(
        "remove it or point {} at a directory you own",
        nested_bazel::ROOT_ENV
    );
    let shown = path.display();
    if is_symlink {
        Some(format!("{shown} is a symlink; {fix}"))
    } else if !is_dir {
        Some(format!("{shown} is not a directory; {fix}"))
    } else if owner != me {
        Some(format!(
            "{shown} is owned by uid {owner}, not uid {me}; {fix}"
        ))
    } else if mode & 0o077 != 0 {
        Some(format!(
            "{shown} is accessible by other users (mode {:04o}); {fix}",
            mode & 0o7777
        ))
    } else {
        None
    }
}

#[cfg(unix)]
fn create_private(base: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    if let Some(parent) = base.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(base) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("cannot create {}: {error}", base.display())),
    }
    let meta = std::fs::symlink_metadata(base)
        .map_err(|error| format!("cannot stat {}: {error}", base.display()))?;
    let kind = meta.file_type();
    match private_dir_error(
        base,
        kind.is_symlink(),
        kind.is_dir(),
        meta.uid(),
        meta.mode(),
        current_uid()?,
    ) {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn create_private(base: &Path) -> Result<(), String> {
    std::fs::create_dir_all(base)
        .map_err(|error| format!("cannot create {}: {error}", base.display()))
}

const DOWNLOADER: &str = "allow bcr.bazel.build\nblock *\n";
const BLOCKED: &str = "Configured URL rewriter blocked all URLs:";

/// Explains a failed offline run whose download the nested downloader blocked.
fn blocked_download(offline: bool, run: &Run, module: Option<&str>) -> Option<String> {
    if !offline || run.status.success() {
        return None;
    }
    let combined = run.combined();
    let line = combined.lines().find(|line| line.contains(BLOCKED))?;
    let mut message = format!(
        "{}\nthe offline nested downloader allows only bcr.bazel.build; set DX_NESTED_BAZEL_NETWORK=1 (nested_bazel_test(network = True) or NESTED_BAZEL_NETWORK_ENV with NESTED_BAZEL_NETWORK_TAGS)",
        line.trim()
    );
    let compact: String = module
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if compact.contains("local_path_override(module_name=\"rules_dx\"") {
        message.push_str(" or drop local_path_override(module_name = \"rules_dx\")");
    }
    Some(message)
}

fn capture(mut command: Command, what: &str, offline: bool, root: &Path) -> Run {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("run {what}: {error}"));
    let run = Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    let module = std::fs::read_to_string(root.join("MODULE.bazel")).ok();
    if let Some(message) = blocked_download(offline, &run, module.as_deref()) {
        panic!("nested bazel: {message}");
    }
    run
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);
static RELEASE: Once = Once::new();

#[cfg(any(unix, windows))]
fn release_at_exit() {
    unsafe extern "C" {
        fn atexit(callback: extern "C" fn()) -> i32;
    }
    unsafe {
        atexit(release_hook);
    }
}

#[cfg(not(any(unix, windows)))]
fn release_at_exit() {}

extern "C" fn release_hook() {
    release();
}

/// Stops the shared server of this process and frees its slot.
pub fn release() {
    let taken = match SERVER.try_lock() {
        Ok(mut guard) => guard.take(),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner().take(),
        Err(std::sync::TryLockError::WouldBlock) => None,
    };
    drop(taken);
}

type Lease = RefCell<MutexGuard<'static, Option<Server>>>;

thread_local! {
    static HELD: RefCell<Weak<Lease>> = const { RefCell::new(Weak::new()) };
}

static HOME: Mutex<Option<(PathBuf, bool)>> = Mutex::new(None);

/// One test case holding the shared server and its emptied workspace.
pub struct Case {
    lease: Rc<Lease>,
    root: PathBuf,
}

/// Starts a case in the workspace `name`; cases run one at a time and share one server.
pub fn case(name: &str) -> Case {
    let held = HELD.with(|held| held.borrow().upgrade());
    let lease = held.unwrap_or_else(|| {
        let mut guard = SERVER.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            let config = Config::from_env().unwrap_or_else(|error| panic!("nested bazel: {error}"));
            let server =
                Server::acquire(config).unwrap_or_else(|error| panic!("nested bazel: {error}"));
            *HOME.lock().unwrap_or_else(PoisonError::into_inner) =
                Some((server.home().to_owned(), server.config.offline));
            *guard = Some(server);
            RELEASE.call_once(release_at_exit);
        }
        let lease = Rc::new(RefCell::new(guard));
        HELD.with(|held| *held.borrow_mut() = Rc::downgrade(&lease));
        lease
    });
    let root = match lease.borrow_mut().as_mut() {
        Some(server) => server
            .enter(name)
            .unwrap_or_else(|error| panic!("nested bazel: {error}")),
        None => panic!("nested bazel: no server"),
    };
    Case { lease, root }
}

/// Runs `bazel` in `root` through the shared server of this process.
pub fn bazel_in(root: &Path, args: &[&str]) -> Run {
    let (home, offline) = HOME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| panic!("nested bazel: no case has started"));
    capture(
        bazel_command_at(&home, root, args, &[]),
        "nested bazel",
        offline,
        root,
    )
}

impl Case {
    fn server(&self) -> Ref<'_, Server> {
        Ref::map(self.lease.borrow(), |guard| match guard.as_ref() {
            Some(server) => server,
            None => panic!("nested bazel: the server was released"),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn user_root(&self) -> PathBuf {
        self.server().user_root()
    }

    pub fn servers(&self) -> Vec<u32> {
        self.server().servers()
    }

    /// Builds a command for `bin` that reaches Bazel only through the shared server.
    pub fn command(&self, bin: &Path, envs: &[(&str, &str)]) -> Command {
        self.server()
            .command(bin, envs)
            .unwrap_or_else(|error| panic!("nested bazel: {error}"))
    }

    /// Runs `bin` with the shared server on `PATH`.
    pub fn run(&self, bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> Run {
        let mut command = self.command(bin, envs);
        command.args(args);
        let offline = self.server().config.offline;
        capture(command, &bin.display().to_string(), offline, &self.root)
    }

    /// Runs `bazel` in the workspace with the startup options dx uses.
    pub fn bazel(&self, args: &[&str]) -> Run {
        bazel_in(&self.root, args)
    }

    /// Runs `bazel` in the workspace with extra environment variables.
    pub fn bazel_with(&self, args: &[&str], envs: &[(&str, &str)]) -> Run {
        let command = bazel_command_at(self.server().home(), &self.root, args, envs);
        let offline = self.server().config.offline;
        capture(command, "nested bazel", offline, &self.root)
    }

    /// Builds an unspawned `bazel` command in the workspace with the startup options dx uses.
    pub fn bazel_command(&self, args: &[&str]) -> Command {
        bazel_command_at(self.server().home(), &self.root, args, &[])
    }
}

impl std::ops::Deref for Case {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.root
    }
}

/// One recorded Bazel reply, matched on the exact argument line.
pub struct Reply<'a> {
    pub args: &'a str,
    pub stdout: &'a str,
    pub stderr: &'a str,
    pub exit: i32,
}

/// A `bazel` on `PATH` that replays recorded output and logs every call.
pub struct FakeBazel {
    bin: PathBuf,
    log: PathBuf,
}

impl FakeBazel {
    pub fn new(dir: &Path, replies: &[Reply]) -> FakeBazel {
        let bin = dir.join("bin");
        let log = dir.join("calls.log");
        let mut script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\ncase \"$*\" in\n",
            shell_quote(&log.to_string_lossy())
        );
        for (index, reply) in replies.iter().enumerate() {
            let out = dir.join(format!("reply-{index}.out"));
            let err = dir.join(format!("reply-{index}.err"));
            write(&out, reply.stdout).unwrap_or_else(|error| panic!("{error}"));
            write(&err, reply.stderr).unwrap_or_else(|error| panic!("{error}"));
            script.push_str(&format!(
                "{})\n  cat {}\n  cat {} >&2\n  exit {}\n  ;;\n",
                shell_quote(reply.args),
                shell_quote(&out.to_string_lossy()),
                shell_quote(&err.to_string_lossy()),
                reply.exit
            ));
        }
        script.push_str("esac\nprintf 'fake bazel has no reply for: %s\\n' \"$*\" >&2\nexit 2\n");
        let fake = bin.join("bazel");
        write(&fake, &script).unwrap_or_else(|error| panic!("{error}"));
        make_executable(&fake).unwrap_or_else(|error| panic!("{error}"));
        write(&log, "").unwrap_or_else(|error| panic!("{error}"));
        FakeBazel { bin, log }
    }

    /// Returns `PATH` with the fake first.
    pub fn path(&self) -> String {
        let mut dirs = vec![self.bin.clone()];
        dirs.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(dirs)
            .unwrap_or_else(|error| panic!("PATH: {error}"))
            .to_string_lossy()
            .into_owned()
    }

    /// Lists the argument lines of every call so far.
    pub fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn scratch(prefix: &str) -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("scratch tempdir");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private scratch");
        dir
    }

    fn config(base: &Path, bazel: &Path) -> Config {
        Config {
            base: base.to_path_buf(),
            slots: 2,
            bazel: bazel.to_path_buf(),
            heap: DEFAULT_HEAP.to_owned(),
            disk_cache: PathBuf::from("/caches/disk"),
            repository_cache: None,
            offline: false,
        }
    }

    fn recorder(dir: &Path) -> (PathBuf, PathBuf) {
        let log = dir.join("real.log");
        let bazel = dir.join("real-bazel");
        std::fs::write(
            &bazel,
            format!(
                "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$PWD\" \"${{TEST_TMPDIR:-unset}}\" \"$*\" >> '{}'\n",
                log.display()
            ),
        )
        .expect("write recorder");
        make_executable(&bazel).expect("chmod");
        (bazel, log)
    }

    fn lookup(vars: Vec<(&'static str, OsString)>) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn resolve(key: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(key))
    }

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Fixture {
            let dir = scratch("nested-config-");
            std::fs::write(dir.path().join("bazel"), "").expect("write bazel");
            Fixture { dir }
        }

        fn base(&self) -> PathBuf {
            self.dir.path().join("base")
        }

        fn config(&self, vars: &[(&'static str, &str)], rc: &str, home: Option<&Path>) -> Config {
            let mut all = vec![
                ("DX_NESTED_BAZEL_ROOT", self.base().into_os_string()),
                (
                    "DX_NESTED_BAZEL",
                    self.dir.path().join("bazel").into_os_string(),
                ),
            ];
            all.extend(
                vars.iter()
                    .map(|(key, value)| (*key, OsString::from(value))),
            );
            Config::from_parts(&lookup(all), &resolve, rc, home).expect("config")
        }
    }

    const BLOCKED_LINE: &str = "ERROR: Configured URL rewriter blocked all URLs: [https://github.com/astral-sh/python-build-standalone/releases/download/20250317/SHA256SUMS]";

    fn finished(code: i32, stderr: &str) -> Run {
        use std::os::unix::process::ExitStatusExt;
        Run {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
    }

    #[test]
    fn blocked_download_names_the_network_and_the_override() {
        let run = finished(1, &format!("Loading\n{BLOCKED_LINE}\nFAILED\n"));
        let module = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"/r\")\n";
        let message = blocked_download(true, &run, Some(module)).expect("message");
        assert!(message.starts_with(BLOCKED_LINE.trim()), "{message}");
        assert!(message.contains("SHA256SUMS"), "{message}");
        assert!(message.contains("DX_NESTED_BAZEL_NETWORK=1"), "{message}");
        assert!(
            message.contains("drop local_path_override(module_name = \"rules_dx\")"),
            "{message}"
        );
    }

    #[test]
    fn blocked_download_follows_the_module_for_the_override_hint() {
        let run = finished(1, BLOCKED_LINE);
        let plain = blocked_download(true, &run, Some("module(name = \"c\")\n")).expect("plain");
        assert!(plain.contains("DX_NESTED_BAZEL_NETWORK=1"), "{plain}");
        assert!(!plain.contains("local_path_override"), "{plain}");
        let missing = blocked_download(true, &run, None).expect("missing");
        assert!(!missing.contains("local_path_override"), "{missing}");
        let multi =
            "local_path_override(\n    module_name = \"rules_dx\",\n    path = \"/r\",\n)\n";
        let multi = blocked_download(true, &run, Some(multi)).expect("multi");
        assert!(multi.contains("DX_NESTED_BAZEL_NETWORK=1"), "{multi}");
        assert!(multi.contains("drop local_path_override"), "{multi}");
        let other = "local_path_override(module_name = \"rules_go\", path = \"/g\")\n";
        let other = blocked_download(true, &run, Some(other)).expect("other");
        assert!(!other.contains("local_path_override"), "{other}");
    }

    #[test]
    fn blocked_download_is_silent_online_on_success_and_without_the_marker() {
        let module = Some("local_path_override(module_name = \"rules_dx\", path = \"/r\")\n");
        assert_eq!(
            blocked_download(false, &finished(1, BLOCKED_LINE), module),
            None
        );
        assert_eq!(
            blocked_download(true, &finished(0, BLOCKED_LINE), module),
            None
        );
        assert_eq!(
            blocked_download(true, &finished(1, "ERROR: no such target\n"), module),
            None
        );
    }

    #[test]
    fn capture_panics_on_an_offline_blocked_download() {
        let dir = scratch("nested-blocked-");
        std::fs::write(
            dir.path().join("MODULE.bazel"),
            "local_path_override(module_name = \"rules_dx\", path = \"/r\")\n",
        )
        .expect("module");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", &format!("echo '{BLOCKED_LINE}' >&2; exit 1")]);
        let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capture(command, "sh", true, dir.path())
        })) {
            Ok(run) => panic!("no panic: {}", run.combined()),
            Err(panic) => panic.downcast_ref::<String>().cloned().unwrap_or_default(),
        };
        assert!(message.starts_with("nested bazel: "), "{message}");
        assert!(message.contains("DX_NESTED_BAZEL_NETWORK=1"), "{message}");
        let mut online = Command::new("/bin/sh");
        online.args(["-c", &format!("echo '{BLOCKED_LINE}' >&2; exit 1")]);
        let run = capture(online, "sh", false, dir.path());
        assert!(!run.status.success());
    }

    #[test]
    fn the_config_is_offline_unless_the_network_is_asked_for() {
        let fixture = Fixture::new();
        assert!(fixture.config(&[], "", None).offline);
        assert!(
            fixture
                .config(&[("DX_NESTED_BAZEL_NETWORK", "")], "", None)
                .offline
        );
        assert!(
            !fixture
                .config(&[("DX_NESTED_BAZEL_NETWORK", "1")], "", None)
                .offline
        );
    }

    #[test]
    fn the_config_falls_back_to_three_slots_on_zero_or_garbage() {
        let fixture = Fixture::new();
        assert_eq!(fixture.config(&[], "", None).slots, 3);
        assert_eq!(fixture.config(&[(SLOTS_ENV, "0")], "", None).slots, 3);
        assert_eq!(fixture.config(&[(SLOTS_ENV, "abc")], "", None).slots, 3);
        assert_eq!(fixture.config(&[(SLOTS_ENV, "2")], "", None).slots, 2);
    }

    #[test]
    fn the_config_takes_the_heap_override() {
        let fixture = Fixture::new();
        assert_eq!(fixture.config(&[], "", None).heap, "1536m");
        assert_eq!(
            fixture
                .config(&[("DX_NESTED_BAZEL_HEAP", "")], "", None)
                .heap,
            "1536m"
        );
        assert_eq!(
            fixture
                .config(&[("DX_NESTED_BAZEL_HEAP", "4g")], "", None)
                .heap,
            "4g"
        );
    }

    #[test]
    fn the_config_caches_prefer_the_environment_then_the_rc_then_the_home() {
        let fixture = Fixture::new();
        let home = fixture.dir.path().join("home");
        let repos = home.join(".cache/bazel/_bazel_x/cache/repos/v1");
        std::fs::create_dir_all(&repos).expect("mkdir repos");
        let rc = "common --disk_cache=~/rc
common --repository_cache=/rc/repos
";
        let env = [
            ("DX_NESTED_BAZEL_DISK_CACHE", "/env/disk"),
            ("DX_NESTED_BAZEL_REPOSITORY_CACHE", "/env/repos"),
        ];

        let chosen = fixture.config(&env, rc, Some(&home));
        assert_eq!(chosen.disk_cache, PathBuf::from("/env/disk"));
        assert_eq!(chosen.repository_cache, Some(PathBuf::from("/env/repos")));

        let chosen = fixture.config(&[], rc, Some(&home));
        assert_eq!(chosen.disk_cache, home.join("rc"));
        assert_eq!(chosen.repository_cache, Some(PathBuf::from("/rc/repos")));

        let chosen = fixture.config(&[], "", Some(&home));
        assert_eq!(chosen.disk_cache, fixture.base().join("disk-cache"));
        assert_eq!(chosen.repository_cache, Some(repos));

        std::fs::remove_dir_all(home.join(".cache")).expect("remove cache");
        assert_eq!(fixture.config(&[], "", Some(&home)).repository_cache, None);
    }

    #[test]
    fn a_process_is_owned_only_when_its_command_line_names_the_user_root() {
        let root = Path::new("/nested/owned/root");
        let mut child = Command::new("sh")
            .args(["-c", "sleep 30; true", &root.to_string_lossy()])
            .spawn()
            .expect("spawn");
        let pid = child.id();
        let started = Instant::now();
        while !owned_by(pid, root) && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let owned = owned_by(pid, root);
        let other = owned_by(pid, Path::new("/nested/other/root"));
        let unrelated = owned_by(std::process::id(), root);
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert!(owned);
        assert!(!other);
        assert!(!unrelated);
        assert!(!owned_by(pid, root));
    }

    #[test]
    fn rc_paths_take_the_last_value_and_expand_home() {
        let rc = "# common --disk_cache=/ignored\ncommon --disk_cache=/one\nbuild --disk_cache=~/two --jobs=2\ncommon --repository_cache=relative\n";
        let home = Path::new("/home/me");
        assert_eq!(
            rc_path(rc, "disk_cache", Some(home)),
            Some(PathBuf::from("/home/me/two"))
        );
        assert_eq!(rc_path(rc, "repository_cache", Some(home)), None);
        assert_eq!(rc_path(rc, "disk_cache", None), Some(PathBuf::from("/one")));
    }

    #[test]
    fn the_disk_cache_prefers_the_environment_then_the_rc_then_the_base() {
        let rc = "common --disk_cache=~/rc\n";
        let home = Some(Path::new("/home/me"));
        let base = Path::new("/b");
        assert_eq!(
            disk_cache(Some(PathBuf::from("/env")), rc, home, base),
            PathBuf::from("/env")
        );
        assert_eq!(
            disk_cache(None, rc, home, base),
            PathBuf::from("/home/me/rc")
        );
        assert_eq!(
            disk_cache(None, "", home, base),
            PathBuf::from("/b/disk-cache")
        );
    }

    #[test]
    fn the_wrapper_pins_the_startup_options_and_the_rc_names_the_caches() {
        let config = config(Path::new("/b"), Path::new("/usr/bin/bazel"));
        let script = wrapper_script(&config, Path::new("/b/0/u"), Path::new("/b/0/bazelrc"));
        assert_eq!(
            script,
            "#!/bin/sh\nexec '/usr/bin/bazel' '--output_user_root=/b/0/u' '--max_idle_secs=5' '--host_jvm_args=-Xmx1536m' '--bazelrc=/b/0/bazelrc' \"$@\"\n"
        );
        assert_eq!(
            bazelrc(&config, Path::new("/b/0/downloader.cfg")),
            "common --local_resources=cpu=2\ncommon --disk_cache=/caches/disk\n"
        );
        let offline = Config {
            offline: true,
            ..config
        };
        assert_eq!(
            bazelrc(&offline, Path::new("/b/0/downloader.cfg")),
            "common --local_resources=cpu=2\ncommon --downloader_config=/b/0/downloader.cfg\ncommon --disk_cache=/caches/disk\n"
        );
    }

    #[test]
    fn find_bazel_skips_the_harness_wrappers() {
        let dir = scratch("nested-find-");
        let wrapped = dir.path().join("base/0/bin");
        let real = dir.path().join("real");
        for bin in [&wrapped, &real] {
            std::fs::create_dir_all(bin).expect("mkdir");
            std::fs::write(bin.join("bazel"), "").expect("write");
        }
        let path = std::env::join_paths([&wrapped, &real]).expect("join");
        assert_eq!(
            find_bazel(None, None, &path, &dir.path().join("base")).expect("found"),
            real.join("bazel")
        );
    }

    #[test]
    fn find_bazel_prefers_the_override() {
        let dir = scratch("nested-find-override-");
        let real = dir.path().join("real");
        let chosen = dir.path().join("chosen/bazel-9");
        for file in [real.join("bazel"), chosen.clone()] {
            std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
            std::fs::write(&file, "").expect("write");
        }
        let path = std::env::join_paths([&real]).expect("join");
        assert_eq!(
            find_bazel(Some(chosen.clone()), None, &path, &dir.path().join("base")).expect("found"),
            chosen
        );
    }

    #[test]
    fn find_bazel_rejects_a_missing_override() {
        let dir = scratch("nested-find-missing-");
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).expect("mkdir");
        std::fs::write(real.join("bazel"), "").expect("write");
        let missing = dir.path().join("missing/bazel");
        let path = std::env::join_paths([&real]).expect("join");
        let error = find_bazel(Some(missing.clone()), None, &path, &dir.path().join("base"))
            .expect_err("missing override");
        assert_eq!(
            error,
            format!("DX_NESTED_BAZEL {} is not a file", missing.display())
        );
    }

    #[test]
    fn find_bazel_prefers_the_override_over_the_pinned_binary() {
        let dir = scratch("nested-find-pinned-override-");
        let chosen = dir.path().join("chosen/bazel");
        let pinned = dir.path().join("pinned/bazel");
        for file in [&chosen, &pinned] {
            std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
            std::fs::write(file, "").expect("write");
        }
        assert_eq!(
            find_bazel(
                Some(chosen.clone()),
                Some(pinned),
                std::ffi::OsStr::new(""),
                &dir.path().join("base")
            )
            .expect("found"),
            chosen
        );
    }

    #[test]
    fn find_bazel_prefers_the_pinned_binary_over_path() {
        let dir = scratch("nested-find-pinned-");
        let real = dir.path().join("real");
        let pinned = dir.path().join("pinned/bazel");
        for file in [real.join("bazelisk"), pinned.clone()] {
            std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
            std::fs::write(&file, "").expect("write");
        }
        let path = std::env::join_paths([&real]).expect("join");
        assert_eq!(
            find_bazel(None, Some(pinned.clone()), &path, &dir.path().join("base")).expect("found"),
            pinned
        );
    }

    #[test]
    fn find_bazel_rejects_a_missing_pinned_binary() {
        let dir = scratch("nested-find-pinned-missing-");
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).expect("mkdir");
        std::fs::write(real.join("bazel"), "").expect("write");
        let missing = dir.path().join("missing/bazel");
        let path = std::env::join_paths([&real]).expect("join");
        let error = find_bazel(None, Some(missing.clone()), &path, &dir.path().join("base"))
            .expect_err("missing pinned");
        assert_eq!(
            error,
            format!("DX_NESTED_BAZEL_PINNED {} is not a file", missing.display())
        );
    }

    #[test]
    fn config_reports_a_pinned_key_missing_from_the_runfiles() {
        let fixture = Fixture::new();
        let all = vec![
            ("DX_NESTED_BAZEL_ROOT", fixture.base().into_os_string()),
            (PINNED_ENV, OsString::from("nested_bazel_x/file/bazel")),
        ];
        let error = Config::from_parts(
            &lookup(all),
            &|key| Err(format!("{PINNED_ENV} {key} is not in the runfiles")),
            "",
            None,
        )
        .expect_err("unresolved pinned");
        assert_eq!(
            error,
            "DX_NESTED_BAZEL_PINNED nested_bazel_x/file/bazel is not in the runfiles"
        );
    }

    #[test]
    fn config_uses_the_resolved_pinned_binary() {
        let fixture = Fixture::new();
        let pinned = fixture.dir.path().join("bazel");
        let all = vec![
            ("DX_NESTED_BAZEL_ROOT", fixture.base().into_os_string()),
            (PINNED_ENV, pinned.clone().into_os_string()),
            ("PATH", OsString::new()),
        ];
        let config = Config::from_parts(&lookup(all), &resolve, "", None).expect("config");
        assert_eq!(config.bazel, pinned);
    }

    #[test]
    fn resolve_pinned_names_the_variable() {
        let error = resolve_pinned("no_such_repo/file/bazel").expect_err("absent");
        assert!(
            error.starts_with(
                "DX_NESTED_BAZEL_PINNED no_such_repo/file/bazel is not in the runfiles"
            ),
            "{error}"
        );
    }

    #[test]
    fn reset_empties_the_workspace_and_removes_links_without_following_them() {
        let dir = scratch("nested-reset-");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).expect("mkdir");
        std::fs::write(outside.join("keep"), "").expect("write");
        let root = dir.path().join("ws");
        std::fs::create_dir_all(root.join("pkg/locked")).expect("mkdir");
        std::fs::write(root.join("MODULE.bazel"), "").expect("write");
        std::os::unix::fs::symlink(&outside, root.join("bazel-out")).expect("link");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                root.join("pkg/locked"),
                std::fs::Permissions::from_mode(0o500),
            )
            .expect("chmod");
        }
        reset(&root).expect("reset");
        assert_eq!(std::fs::read_dir(&root).expect("list").count(), 0);
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn acquire_refuses_a_base_other_users_can_write() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("base-open");
        let base = dir.path().join("base");
        std::fs::create_dir(&base).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o777)).unwrap();
        let cfg = config(&base, Path::new("/bin/true"));
        assert!(Server::acquire(cfg).is_err());
        assert!(!base.join("0").exists());
        let mode = std::fs::metadata(&base).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o777);
    }

    #[test]
    fn acquire_refuses_a_base_that_is_a_symlink() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("base-link");
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
        let base = dir.path().join("base");
        std::os::unix::fs::symlink(&target, &base).unwrap();
        let cfg = config(&base, Path::new("/bin/true"));
        assert!(Server::acquire(cfg).is_err());
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    }

    #[test]
    fn acquire_refuses_a_base_that_is_a_file() {
        let dir = scratch("base-file");
        let base = dir.path().join("base");
        std::fs::write(&base, "x").unwrap();
        let cfg = config(&base, Path::new("/bin/true"));
        assert!(Server::acquire(cfg).is_err());
    }

    #[test]
    fn acquire_creates_a_missing_base_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("base-new");
        let base = dir.path().join("missing").join("base");
        create_private(&base).unwrap();
        let mode = std::fs::metadata(&base).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn acquire_reuses_an_existing_private_base() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("base-reuse");
        let base = dir.path().join("base");
        std::fs::create_dir(&base).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        create_private(&base).unwrap();
        create_private(&base).unwrap();
    }

    #[test]
    fn private_dir_error_cases() {
        let p = Path::new("/tmp/dxnb-1");
        assert!(private_dir_error(p, false, true, 5, 0o40700, 5).is_none());
        assert!(private_dir_error(p, false, true, 6, 0o40700, 5).is_some());
        assert!(private_dir_error(p, false, true, 5, 0o40750, 5).is_some());
        assert!(private_dir_error(p, false, true, 5, 0o40705, 5).is_some());
        assert!(private_dir_error(p, true, false, 5, 0o40700, 5).is_some());
        assert!(private_dir_error(p, false, false, 5, 0o100600, 5).is_some());
        let msg = private_dir_error(p, false, true, 5, 0o40777, 5).unwrap();
        assert!(msg.contains(nested_bazel::ROOT_ENV), "{msg}");
    }

    #[test]
    fn current_uid_matches_a_fresh_dir_owner() {
        use std::os::unix::fs::MetadataExt;
        let dir = scratch("uid");
        assert_eq!(
            current_uid().unwrap(),
            std::fs::metadata(dir.path()).unwrap().uid()
        );
    }

    #[test]
    fn slots_are_exclusive_and_commands_go_through_the_wrapper() {
        let dir = scratch("nested-slot-");
        let (bazel, log) = recorder(dir.path());
        let base = dir.path().join("base");
        let mut first = Server::acquire(config(&base, &bazel)).expect("first slot");
        let second = Server::acquire(config(&base, &bazel)).expect("second slot");
        assert_ne!(first.home(), second.home());
        drop(second);
        let third = Server::acquire(config(&base, &bazel)).expect("freed slot");
        assert_ne!(first.home(), third.home());
        assert_eq!(
            std::fs::read_to_string(first.home().join("downloader.cfg")).expect("downloader"),
            DOWNLOADER
        );
        assert_eq!(
            std::fs::read_to_string(first.home().join("bazelrc")).expect("bazelrc"),
            bazelrc(&first.config, &first.home().join("downloader.cfg"))
        );

        let root = first.enter("ws").expect("enter");
        std::fs::write(root.join("MODULE.bazel"), "").expect("write");
        let run = capture(
            first.bazel_command(&root, &["info", "release"]),
            "bazel",
            true,
            &root,
        );
        assert!(run.status.success(), "{}", run.combined());
        let calls = std::fs::read_to_string(&log).expect("log");
        let user_root = first.user_root();
        assert_eq!(
            calls,
            format!(
                "{}|unset|--output_user_root={} --max_idle_secs=5 --host_jvm_args=-Xmx1536m --bazelrc={} --nohome_rc --nosystem_rc info release\n",
                root.display(),
                user_root.display(),
                first.home().join("bazelrc").display()
            )
        );
        let again = first.enter("ws").expect("enter again");
        assert_eq!(again, root);
        assert_eq!(std::fs::read_dir(&root).expect("list").count(), 0);
    }

    #[test]
    fn commands_strip_the_test_environment_and_put_the_wrapper_first() {
        let dir = scratch("nested-env-");
        let (bazel, _) = recorder(dir.path());
        let server = Server::acquire(config(&dir.path().join("base"), &bazel)).expect("slot");
        let env = Path::new("/usr/bin/env");
        let command = server
            .command(env, &[("PATH", "/custom"), ("KEEP", "1")])
            .expect("command");
        let run = capture(command, "env", false, dir.path());
        let lines: Vec<&str> = run.stdout.lines().collect();
        assert!(lines.contains(&"KEEP=1"), "{}", run.stdout);
        let wrapper_dir = server.home().join("bin");
        assert!(
            lines.contains(&format!("PATH={}:/custom", wrapper_dir.display()).as_str()),
            "{}",
            run.stdout
        );
        assert!(
            !lines.iter().any(|line| line.starts_with("TEST_")),
            "{}",
            run.stdout
        );
    }

    #[test]
    fn fake_bazel_replays_exact_calls_and_logs_them() {
        let dir = scratch("nested-fake-");
        let fake = FakeBazel::new(
            dir.path(),
            &[Reply {
                args: "--nohome_rc query -- tests('//a')",
                stdout: "rule //a:t\n",
                stderr: "INFO: done\n",
                exit: 3,
            }],
        );
        let bazel = dir.path().join("bin/bazel");
        let hit = Command::new(&bazel)
            .args(["--nohome_rc", "query", "--", "tests('//a')"])
            .output()
            .expect("fake runs");
        assert_eq!(hit.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&hit.stdout), "rule //a:t\n");
        assert_eq!(String::from_utf8_lossy(&hit.stderr), "INFO: done\n");
        let miss = Command::new(&bazel)
            .arg("build")
            .output()
            .expect("fake runs");
        assert_eq!(miss.status.code(), Some(2));
        assert_eq!(
            fake.calls(),
            vec![
                "--nohome_rc query -- tests('//a')".to_owned(),
                "build".to_owned()
            ]
        );
        assert!(fake
            .path()
            .starts_with(&*dir.path().join("bin").to_string_lossy()));
    }

    fn stand_in(workspace: &Path) -> Child {
        Command::new("sh")
            .args([
                "-c",
                "trap 'kill $p; exit' TERM; sleep 60 & p=$!; wait $p",
                "bazel-server",
            ])
            .arg(format!("--workspace_directory={}", workspace.display()))
            .arg("--output_base=/home/u/.cache/bazel/_bazel_u/abc")
            .spawn()
            .expect("spawn stand-in server")
    }

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let started = Instant::now();
        while !done() {
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn stop(mut child: Child, workspaces: &Path) {
        assert!(kill_servers_in_workspaces(workspaces).is_empty());
        let _ = child.wait();
    }

    #[test]
    fn servers_with_a_foreign_output_root_are_found_and_stopped_by_workspace() {
        let dir = scratch("nested-stray-");
        let workspaces = dir.path().join("0/w");
        let outside_root = dir.path().join("1/w");
        let mut stray = stand_in(&workspaces.join("consumer"));
        let other = stand_in(&outside_root.join("consumer"));
        let (stray_pid, other_pid) = (stray.id(), other.id());
        wait_until("both stand-ins", || {
            servers_in_workspaces(&workspaces) == vec![stray_pid]
                && servers_in_workspaces(&outside_root) == vec![other_pid]
        });
        assert!(kill_servers_in_workspaces(&workspaces).is_empty());
        let _ = stray.wait();
        assert!(!alive(stray_pid));
        assert!(alive(other_pid));
        stop(other, &outside_root);
    }

    #[test]
    fn shutdown_stops_a_server_outside_the_pinned_output_root() {
        let dir = scratch("nested-shutdown-");
        let mut server =
            Server::acquire(config(dir.path(), Path::new("/bin/true"))).expect("acquire");
        let mut stray = stand_in(&server.home().join("w/consumer"));
        let pid = stray.id();
        wait_until("the stand-in", || server.servers() == vec![pid]);
        assert!(server.shutdown().is_empty());
        let _ = stray.wait();
        assert!(!alive(pid));
    }

    #[test]
    fn drop_stops_a_server_outside_the_pinned_output_root() {
        let dir = scratch("nested-drop-");
        let server = Server::acquire(config(dir.path(), Path::new("/bin/true"))).expect("acquire");
        let mut stray = stand_in(&server.home().join("w/consumer"));
        let pid = stray.id();
        wait_until("the stand-in", || server.servers() == vec![pid]);
        drop(server);
        let _ = stray.wait();
        assert!(!alive(pid));
    }

    #[test]
    fn claiming_a_slot_stops_a_server_left_outside_the_pinned_output_root() {
        let dir = scratch("nested-claim-");
        let workspace = dir.path().join("0/w/consumer");
        std::fs::create_dir_all(&workspace).expect("mkdir");
        let mut stray = stand_in(&workspace);
        let pid = stray.id();
        wait_until("the stand-in", || {
            servers_in_workspaces(&dir.path().join("0/w")) == vec![pid]
        });
        let server = Server::acquire(config(dir.path(), Path::new("/bin/true"))).expect("acquire");
        let _ = stray.wait();
        assert!(!alive(pid));
        drop(server);
    }

    #[cfg(unix)]
    #[test]
    fn the_reaper_stops_servers_after_the_owner_is_killed() {
        let dir = scratch("nested-reaper-");
        let home = dir.path().join("0");
        let workspaces = home.join("w");
        std::fs::create_dir_all(&workspaces).expect("mkdir");
        let lock = File::create(home.join("lock")).expect("lock");
        let mut owner = Command::new("sleep").arg("60").spawn().expect("owner");
        let mut stray = stand_in(&workspaces.join("consumer"));
        let pid = stray.id();
        wait_until("the stand-in", || {
            servers_in_workspaces(&workspaces) == vec![pid]
        });
        let mut reaper = spawn_reaper(owner.id(), &home, &lock).expect("reaper");
        std::thread::sleep(Duration::from_millis(1500));
        assert!(alive(pid));
        owner.kill().expect("kill owner");
        let _ = owner.wait();
        wait_until("the reaper to stop the stand-in", || !alive(pid));
        let _ = stray.wait();
        let _ = reaper.wait();
    }
}
