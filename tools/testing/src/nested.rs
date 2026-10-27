//! Runs nested Bazel servers for tests: slot leasing, hermetic environments and the fake Bazel.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::time::{Duration, Instant};

use dx_path::nested_bazel;

use dx_testing::Run;

pub const MAX_IDLE_SECS: u32 = 5;
pub const DEFAULT_HEAP: &str = "1536m";
pub const DEFAULT_SLOTS: usize = nested_bazel::DEFAULT_SLOTS as usize;
pub use nested_bazel::SLOTS_ENV;
pub const PINNED_ENV: &str = "DX_NESTED_BAZEL_PINNED";
pub const SHIM_ENV: &str = "DX_NESTED_BAZEL_SHIM";
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
    /// The native `bazel` shim installed in each slot; without it Unix falls back to a sh wrapper.
    pub shim: Option<PathBuf>,
}

impl Config {
    /// Reads the settings from the environment and the outer `~/.bazelrc`.
    pub fn from_env() -> Result<Config, String> {
        let home = pick_home(&|name| std::env::var_os(name));
        let rc = home
            .as_ref()
            .and_then(|home| std::fs::read_to_string(home.join(".bazelrc")).ok())
            .unwrap_or_default();
        Self::from_parts(
            &|name| std::env::var_os(name),
            &resolve_runfile,
            &rc,
            home.as_deref(),
        )
    }

    /// Builds the settings from an environment lookup, the outer bazelrc text and the home.
    fn from_parts(
        lookup: &dyn Fn(&str) -> Option<OsString>,
        resolve: &dyn Fn(&str, &str) -> Result<PathBuf, String>,
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
            Some(key) if chosen.is_none() => Some(resolve(PINNED_ENV, &key)?),
            _ => None,
        };
        let bazel = find_bazel(chosen, pinned, &path, &base)?;
        let shim = match text(SHIM_ENV).filter(|key| !key.is_empty()) {
            Some(key) => Some(resolve(SHIM_ENV, &key)?),
            None => None,
        };
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
            shim,
        })
    }
}

/// Resolves the runfiles key in the variable `name` to an absolute path.
fn resolve_runfile(name: &str, key: &str) -> Result<PathBuf, String> {
    let found = dx_testing::rlocation(key)
        .map_err(|error| format!("{name} {key} is not in the runfiles: {error}"))?;
    std::path::absolute(&found).map_err(|error| format!("{name} {}: {error}", found.display()))
}

/// Returns `HOME`, else `USERPROFILE` (Windows), ignoring empty values.
fn pick_home(lookup: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env_path(lookup, "HOME").or_else(|| env_path(lookup, "USERPROFILE"))
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
    let dir = dx_testing::mkscratch("nested-bazel-uid")
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
    Ok(nested_bazel::root(None, Some(current_uid()?), None))
}

#[cfg(not(unix))]
fn default_base() -> Result<PathBuf, String> {
    let profile = env_path(&|name| std::env::var_os(name), "USERPROFILE");
    Ok(nested_bazel::root(None, None, profile.as_deref()))
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

/// Finds the repository cache under the default output roots: `~/.cache/bazel` (Unix), then the home (Windows).
fn default_repository_cache(home: &Path) -> Option<PathBuf> {
    [home.join(".cache/bazel"), home.to_path_buf()]
        .iter()
        .find_map(|dir| {
            let mut roots: Vec<PathBuf> = std::fs::read_dir(dir)
                .ok()?
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("_bazel_"))
                .map(|entry| entry.path().join("cache/repos/v1"))
                .filter(|path| path.is_dir())
                .collect();
            roots.sort();
            roots.into_iter().next()
        })
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
    for name in ["bazel", "bazelisk", "bazel.exe", "bazelisk.exe"] {
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
            dx_process::make_writable(&path);
            std::fs::remove_dir_all(&path)
        } else {
            // On Windows a directory symlink or junction is removed with remove_dir, never followed.
            std::fs::remove_file(&path).or_else(|error| {
                if cfg!(windows) {
                    std::fs::remove_dir(&path).map_err(|_| error)
                } else {
                    Err(error)
                }
            })
        };
        removed.map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
    }
    Ok(())
}

/// The real Bazel followed by every pinned nested startup option; the one source for both wrappers.
pub fn wrapper_argv(config: &Config, user_root: &Path, rc: &Path) -> Vec<String> {
    vec![
        config.bazel.to_string_lossy().into_owned(),
        format!("--output_user_root={}", user_root.display()),
        format!("--max_idle_secs={MAX_IDLE_SECS}"),
        format!("--host_jvm_args=-Xmx{}", config.heap),
        format!("--bazelrc={}", rc.display()),
    ]
}

/// Renders the sh `bazel` wrapper that pins every nested startup option and logs each call.
pub fn wrapper_script(config: &Config, user_root: &Path, rc: &Path, calls: &Path) -> String {
    let quoted: Vec<String> = wrapper_argv(config, user_root, rc)
        .iter()
        .map(|arg| shell_quote(arg))
        .collect();
    format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\nexec {} \"$@\"\n",
        shell_quote(&calls.to_string_lossy()),
        quoted.join(" ")
    )
}

/// Renders the shim's sidecar `.args` file: one argument per line, the real Bazel first.
pub fn shim_args(argv: &[String]) -> Result<String, String> {
    let mut text = String::new();
    for arg in argv {
        if arg.contains(['\n', '\r']) {
            return Err(format!(
                "nested Bazel startup argument has a line break: {arg:?}"
            ));
        }
        text.push_str(arg);
        text.push('\n');
    }
    Ok(text)
}

/// Parses a shim `.args` file and appends the forwarded arguments: (program, args).
pub fn shim_argv(text: &str, forwarded: &[OsString]) -> Option<(OsString, Vec<OsString>)> {
    let mut lines = text
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    let program = OsString::from(lines.next().filter(|line| !line.is_empty())?);
    let mut args: Vec<OsString> = lines
        .filter(|line| !line.is_empty())
        .map(OsString::from)
        .collect();
    args.extend(forwarded.iter().cloned());
    Some((program, args))
}

/// The `main` of the native `bazel` shim: runs the real Bazel from `<self>.args` with the forwarded args.
pub fn shim_main() -> i32 {
    let me = match std::env::current_exe() {
        Ok(me) => me,
        Err(error) => {
            eprintln!("nested bazel shim: cannot find itself: {error}");
            return 2;
        }
    };
    let sidecar = me.with_extension("args");
    let text = match std::fs::read_to_string(&sidecar) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "nested bazel shim: cannot read {}: {error}",
                sidecar.display()
            );
            return 2;
        }
    };
    let forwarded: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some((program, args)) = shim_argv(&text, &forwarded) else {
        eprintln!("nested bazel shim: {} names no Bazel", sidecar.display());
        return 2;
    };
    let mut command = Command::new(&program);
    command.args(&args);
    run_shim(command, &program)
}

#[cfg(unix)]
fn run_shim(mut command: Command, program: &std::ffi::OsStr) -> i32 {
    use std::os::unix::process::CommandExt;
    let error = command.exec();
    eprintln!(
        "nested bazel shim: cannot run {}: {error}",
        program.to_string_lossy()
    );
    127
}

#[cfg(not(unix))]
fn run_shim(mut command: Command, program: &std::ffi::OsStr) -> i32 {
    // No kill-on-close job here: the Bazel server must outlive each client command.
    match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!(
                "nested bazel shim: cannot run {}: {error}",
                program.to_string_lossy()
            );
            127
        }
    }
}

/// Copies the shim to `dst` unless it is already identical, moving a busy old copy aside on Windows.
pub fn install_shim(src: &Path, dst: &Path) -> Result<(), String> {
    let wanted =
        std::fs::read(src).map_err(|error| format!("cannot read {}: {error}", src.display()))?;
    if std::fs::read(dst).is_ok_and(|have| have == wanted) {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        clean_old_shims(parent, dst);
    }
    if std::fs::symlink_metadata(dst).is_ok() {
        make_file_writable(dst);
        if std::fs::remove_file(dst).is_err() {
            // A running bazel.exe cannot be deleted or overwritten, but it can be renamed.
            let aside = old_shim(dst, std::process::id());
            std::fs::rename(dst, &aside)
                .map_err(|error| format!("cannot move {} aside: {error}", dst.display()))?;
        }
    }
    std::fs::write(dst, &wanted)
        .map_err(|error| format!("cannot write {}: {error}", dst.display()))?;
    make_file_writable(dst);
    make_executable(dst)
}

fn old_shim(dst: &Path, pid: u32) -> PathBuf {
    let mut name = dst.file_name().unwrap_or_default().to_owned();
    name.push(format!(".old-{pid}"));
    dst.with_file_name(name)
}

/// Removes the moved-aside copies of earlier claims; a still-running one stays until next time.
fn clean_old_shims(dir: &Path, dst: &Path) {
    let Some(name) = dst
        .file_name()
        .map(|name| format!("{}.old-", name.to_string_lossy()))
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with(&name) {
            make_file_writable(&entry.path());
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn make_file_writable(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(path, permissions);
        }
    }
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

#[cfg(windows)]
mod signal {
    use std::ffi::c_void;

    pub const SIGTERM: i32 = 15;
    pub const SIGKILL: i32 = 9;

    type Handle = *mut c_void;

    const SYNCHRONIZE: u32 = 0x0010_0000;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const WAIT_TIMEOUT: u32 = 0x0000_0102;

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> Handle;
        fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
        fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    /// Terminates `pid`; Windows has no graceful signal for a console-less server, so both kill.
    pub fn send(pid: u32, _sig: i32) -> bool {
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let done = unsafe { TerminateProcess(handle, 1) } != 0;
        unsafe {
            CloseHandle(handle);
        }
        done
    }

    /// A process is alive while its handle is unsignaled; an exit code of 259 cannot fool this.
    pub fn alive(pid: u32) -> bool {
        let handle =
            unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let waited = unsafe { WaitForSingleObject(handle, 0) };
        unsafe {
            CloseHandle(handle);
        }
        waited == WAIT_TIMEOUT
    }
}

#[cfg(not(any(unix, windows)))]
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

#[cfg(unix)]
fn command_line(pid: u32) -> Option<String> {
    if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) {
        return Some(String::from_utf8_lossy(&cmdline).into_owned());
    }
    if Path::new("/proc/self").exists() {
        return None;
    }
    dx_process::probe("ps", ["-ww", "-o", "command=", "-p", &pid.to_string()])
}

#[cfg(unix)]
fn all_pids() -> Vec<u32> {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        return entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
            .collect();
    }
    let Some(text) = dx_process::probe("ps", ["-A", "-o", "pid="]) else {
        return Vec::new();
    };
    text.split_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

/// Lists Windows processes that look like Bazel servers, one `pid<TAB>command line` per line.
#[cfg(windows)]
const WINDOWS_PROCESSES: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*--output_user_root=*' -or $_.CommandLine -like '*--workspace_directory=*' } | ForEach-Object { \"$($_.ProcessId)`t$($_.CommandLine)\" }";

/// Parses `pid<TAB>command line` lines, skipping anything else.
#[cfg(any(windows, test))]
fn parse_process_lines(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let (pid, cmdline) = line.split_once('\t')?;
            let pid = pid.trim().parse::<u32>().ok()?;
            Some((pid, cmdline.trim_end_matches('\r').to_owned()))
        })
        .collect()
}

/// One sweep's view of the process table: Unix reads `/proc` per pid, Windows takes one snapshot.
struct Processes {
    #[cfg(not(unix))]
    table: Vec<(u32, String)>,
}

impl Processes {
    #[cfg(unix)]
    fn snapshot() -> Processes {
        Processes {}
    }

    #[cfg(windows)]
    fn snapshot() -> Processes {
        let text = dx_process::probe(
            "powershell",
            [
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                WINDOWS_PROCESSES,
            ],
        )
        .unwrap_or_default();
        Processes {
            table: parse_process_lines(&text),
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn snapshot() -> Processes {
        Processes { table: Vec::new() }
    }

    #[cfg(unix)]
    fn command_line(&self, pid: u32) -> Option<String> {
        command_line(pid)
    }

    #[cfg(not(unix))]
    fn command_line(&self, pid: u32) -> Option<String> {
        self.table
            .iter()
            .find(|(each, _)| *each == pid)
            .map(|(_, line)| line.clone())
    }

    #[cfg(unix)]
    fn pids(&self) -> Vec<u32> {
        all_pids()
    }

    #[cfg(not(unix))]
    fn pids(&self) -> Vec<u32> {
        self.table.iter().map(|(pid, _)| *pid).collect()
    }
}

/// Normalizes a path or command line for matching: `/` separators, no `\\?\` prefix, and no case on Windows.
fn normalized(text: &str) -> String {
    let text = text.replace('\\', "/");
    let text = text.replace("//?/", "");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Reports whether `cmdline` names `dir` (after `flag`, when given) as a whole path, quoted or not.
fn names_dir(cmdline: &str, flag: &str, dir: &str) -> bool {
    let line = normalized(cmdline);
    let dir = normalized(dir);
    let dir = dir.trim_end_matches('/');
    if dir.is_empty() {
        return false;
    }
    let needle = format!("{flag}{dir}");
    let mut from = 0;
    while let Some(found) = line[from..].find(&needle) {
        let at = from + found;
        let step = needle.chars().next().map_or(1, char::len_utf8);
        let before = line[..at].chars().next_back();
        let after = line[at + needle.len()..].chars().next();
        let starts = flag.is_empty() || before.is_none_or(|c| matches!(c, ' ' | '"' | '\0' | '\t'));
        let ends = after.is_none_or(|c| matches!(c, '/' | '"' | ' ' | '\0' | '\t'));
        if starts && ends {
            return true;
        }
        from = at + step;
    }
    false
}

fn owned_in(processes: &Processes, pid: u32, user_root: &Path) -> bool {
    processes
        .command_line(pid)
        .is_some_and(|line| names_dir(&line, "", &user_root.to_string_lossy()))
}

#[cfg(all(test, unix))]
fn owned_by(pid: u32, user_root: &Path) -> bool {
    owned_in(&Processes::snapshot(), pid, user_root)
}

/// Lists the live Bazel servers under one output user root.
pub fn servers(user_root: &Path) -> Vec<u32> {
    servers_from(&Processes::snapshot(), user_root)
}

fn servers_from(processes: &Processes, user_root: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(user_root) else {
        return Vec::new();
    };
    let mut pids: Vec<u32> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            std::fs::read_to_string(entry.path().join("server/server.pid.txt")).ok()
        })
        .filter_map(|text| text.trim().parse::<u32>().ok())
        .filter(|pid| alive(*pid) && owned_in(processes, *pid, user_root))
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

fn workspace_prefixes(workspaces: &Path) -> Vec<String> {
    let mut prefixes = vec![workspaces.to_string_lossy().into_owned()];
    if let Ok(real) = std::fs::canonicalize(workspaces) {
        prefixes.push(real.to_string_lossy().into_owned());
    }
    prefixes
}

fn in_workspaces(cmdline: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| names_dir(cmdline, "--workspace_directory=", prefix))
}

/// Lists the live Bazel servers whose workspace is under `workspaces`, whatever their output root.
pub fn servers_in_workspaces(workspaces: &Path) -> Vec<u32> {
    servers_in_workspaces_from(&Processes::snapshot(), workspaces)
}

fn servers_in_workspaces_from(processes: &Processes, workspaces: &Path) -> Vec<u32> {
    let prefixes = workspace_prefixes(workspaces);
    let own = std::process::id();
    let mut pids: Vec<u32> = processes
        .pids()
        .into_iter()
        .filter(|pid| *pid != own && alive(*pid))
        .filter(|pid| {
            processes
                .command_line(*pid)
                .is_some_and(|line| in_workspaces(&line, &prefixes))
        })
        .collect();
    pids.sort_unstable();
    pids
}

/// Lists the servers under `user_root` or with a workspace under `workspaces`, from one snapshot.
fn slot_servers(user_root: &Path, workspaces: &Path) -> Vec<u32> {
    let processes = Processes::snapshot();
    let mut pids = servers_from(&processes, user_root);
    pids.extend(servers_in_workspaces_from(&processes, workspaces));
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Reports whether `cmdline` still belongs to the slot, so a reused pid is never terminated.
fn slot_owns(cmdline: &str, user_root: &Path, workspaces: &Path) -> bool {
    names_dir(cmdline, "", &user_root.to_string_lossy())
        || in_workspaces(cmdline, &workspace_prefixes(workspaces))
}

/// Stops every server of the slot and returns the ones that survive.
fn kill_slot_servers(user_root: &Path, workspaces: &Path) -> Vec<u32> {
    kill_pids(slot_servers(user_root, workspaces), &|line| {
        slot_owns(line, user_root, workspaces)
    })
}

/// Stops every server under `user_root` and returns the ones that survive.
pub fn kill_servers(user_root: &Path) -> Vec<u32> {
    let root = user_root.to_string_lossy();
    kill_pids(servers(user_root), &|line| names_dir(line, "", &root))
}

/// Stops every server whose workspace is under `workspaces` and returns the ones that survive.
pub fn kill_servers_in_workspaces(workspaces: &Path) -> Vec<u32> {
    let prefixes = workspace_prefixes(workspaces);
    kill_pids(servers_in_workspaces(workspaces), &|line| {
        in_workspaces(line, &prefixes)
    })
}

/// Stops `pids`; `still_ours` re-checks each command line on Windows, where pids are reused fast.
fn kill_pids(mut pids: Vec<u32>, still_ours: &dyn Fn(&str) -> bool) -> Vec<u32> {
    pids.sort_unstable();
    pids.dedup();
    if pids.is_empty() {
        return pids;
    }
    if cfg!(windows) {
        let processes = Processes::snapshot();
        pids.retain(|pid| {
            processes
                .command_line(*pid)
                .is_some_and(|line| still_ours(&line))
        });
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
        kill_slot_servers(
            &server.user_root(),
            &server.home.join(nested_bazel::WORKSPACES),
        );
        server.reaper = spawn_reaper(std::process::id(), &server.home, &server._slot);
        let rc = server.home.join("bazelrc");
        let downloader = server.home.join("downloader.cfg");
        write(&downloader, DOWNLOADER)?;
        write(&rc, &bazelrc(&server.config, &downloader))?;
        let wrapper = server.wrapper();
        let argv = wrapper_argv(&server.config, &server.user_root(), &rc);
        match &server.config.shim {
            Some(shim) => {
                install_shim(shim, &wrapper)?;
                write(&wrapper.with_extension("args"), &shim_args(&argv)?)?;
            }
            None if cfg!(unix) => {
                write(
                    &wrapper,
                    &wrapper_script(
                        &server.config,
                        &server.user_root(),
                        &rc,
                        &server.calls_log(),
                    ),
                )?;
                make_executable(&wrapper)?;
            }
            None => {
                return Err(format!(
                    "{SHIM_ENV} is not set; nested Bazel needs the native shim on this platform (add NESTED_BAZEL_DATA and NESTED_BAZEL_ENV)"
                ))
            }
        }
        write(&server.calls_log(), "")?;
        Ok(server)
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn user_root(&self) -> PathBuf {
        self.home.join(nested_bazel::USER_ROOT)
    }

    /// Names the append-only log of every argument line the wrapper receives.
    pub fn calls_log(&self) -> PathBuf {
        self.home.join("calls.log")
    }

    /// Returns the argument lines the wrapper logged since the last call, then empties the log.
    pub fn take_calls(&self) -> Vec<String> {
        let log = self.calls_log();
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = write(&log, "");
        text.lines().map(str::to_owned).collect()
    }

    pub fn wrapper(&self) -> PathBuf {
        wrapper_at(&self.home)
    }

    /// Empties the workspace `name` and makes it current, stopping the server of any other one.
    pub fn enter(&mut self, name: &str) -> Result<PathBuf, String> {
        let root = self.home.join(nested_bazel::WORKSPACES).join(name);
        if self.workspace.as_ref() != Some(&root) {
            self.shutdown();
            self.workspace = Some(root.clone());
        }
        reset(&root)?;
        write(&self.calls_log(), "")?;
        Ok(root)
    }

    /// Lists the live servers this slot owns.
    pub fn servers(&self) -> Vec<u32> {
        slot_servers(&self.user_root(), &self.home.join(nested_bazel::WORKSPACES))
    }

    /// Stops the server and returns any that survive.
    pub fn shutdown(&mut self) -> Vec<u32> {
        if let Some(root) = self.workspace.as_ref().filter(|root| root.is_dir()) {
            if !self.servers().is_empty() {
                let _ = self.bazel_command(root, &["shutdown"]).output();
            }
        }
        kill_slot_servers(&self.user_root(), &self.home.join(nested_bazel::WORKSPACES))
    }

    /// Builds a command for `bin` that reaches Bazel only through this server.
    pub fn command(&self, bin: &Path, envs: &[(&str, &str)]) -> Result<Command, String> {
        self.command_with_front(bin, envs, &[])
    }

    /// Like `command`, with the `front` directories ahead of the server's `bin` on `PATH`.
    pub fn command_with_front(
        &self,
        bin: &Path,
        envs: &[(&str, &str)],
        front: &[&Path],
    ) -> Result<Command, String> {
        let mut command = dx_testing::command_for(bin, &[])
            .map_err(|error| format!("cannot prepare {}: {error}", bin.display()))?;
        apply_env_at(&self.home, &mut command, envs, front);
        Ok(command)
    }

    /// Builds a `bazel` command in `root` with the startup options dx uses.
    pub fn bazel_command(&self, root: &Path, args: &[&str]) -> Command {
        bazel_command_at(&self.home, root, args, &[])
    }
}

fn apply_env_at(home: &Path, command: &mut Command, envs: &[(&str, &str)], front: &[&Path]) {
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
    let mut dirs: Vec<PathBuf> = front.iter().map(|dir| dir.to_path_buf()).collect();
    dirs.push(home.join("bin"));
    dirs.extend(std::env::split_paths(&path));
    if let Ok(joined) = std::env::join_paths(dirs) {
        command.env("PATH", joined);
    }
}

/// The `bazel` every nested command runs: `bin/bazel` in the slot, `bin/bazel.exe` on Windows.
fn wrapper_at(home: &Path) -> PathBuf {
    home.join("bin")
        .join(format!("bazel{}", std::env::consts::EXE_SUFFIX))
}

fn bazel_command_at(home: &Path, root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(wrapper_at(home));
    command.current_dir(root).args(STARTUP).args(args);
    apply_env_at(home, &mut command, envs, &[]);
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

    /// Returns the argument lines the wrapper logged since the last call, then empties the log.
    pub fn take_calls(&self) -> Vec<String> {
        self.server().take_calls()
    }

    /// Stops the server and asserts that none survives.
    pub fn shutdown_server(&self) {
        let mut guard = self.lease.borrow_mut();
        let server = guard
            .as_mut()
            .unwrap_or_else(|| panic!("nested bazel: the server was released"));
        let left = server.shutdown();
        assert!(left.is_empty(), "nested bazel: servers survived: {left:?}");
        assert!(
            server.servers().is_empty(),
            "nested bazel: servers survived the shutdown"
        );
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

    /// The server's real `bazel` wrapper. A shim must call it by absolute path, because a bare
    /// `bazel` would find the shim again and recurse.
    pub fn wrapper(&self) -> PathBuf {
        self.server().wrapper()
    }

    /// Builds a command for `bin` with `shim_dir` ahead of the server's `bin` on `PATH`.
    pub fn command_with_shim(&self, bin: &Path, envs: &[(&str, &str)], shim_dir: &Path) -> Command {
        self.server()
            .command_with_front(bin, envs, &[shim_dir])
            .unwrap_or_else(|error| panic!("nested bazel: {error}"))
    }

    /// Runs `bin` with `shim_dir` ahead of the server on `PATH`. The shim must call `wrapper()`
    /// by absolute path. Do not use a non-`exec` shim in interrupt tests: signals stop at it.
    pub fn run_with_shim(
        &self,
        bin: &Path,
        args: &[&str],
        envs: &[(&str, &str)],
        shim_dir: &Path,
    ) -> Run {
        let mut command = self.command_with_shim(bin, envs, shim_dir);
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
            shim: None,
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

    fn resolve(_name: &str, key: &str) -> Result<PathBuf, String> {
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
        let script = wrapper_script(
            &config,
            Path::new("/b/0/u"),
            Path::new("/b/0/bazelrc"),
            Path::new("/b/0/calls.log"),
        );
        assert_eq!(
            script,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '/b/0/calls.log'\nexec '/usr/bin/bazel' '--output_user_root=/b/0/u' '--max_idle_secs=5' '--host_jvm_args=-Xmx1536m' '--bazelrc=/b/0/bazelrc' \"$@\"\n"
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
            &|name, key| Err(format!("{name} {key} is not in the runfiles")),
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
        let error = resolve_runfile(PINNED_ENV, "no_such_repo/file/bazel").expect_err("absent");
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
        assert_eq!(
            first.take_calls(),
            vec!["--nohome_rc --nosystem_rc info release".to_owned()]
        );
        assert!(first.take_calls().is_empty());
        let again = first.enter("ws").expect("enter again");
        assert_eq!(again, root);
        assert!(first.take_calls().is_empty());
        assert_eq!(std::fs::read_dir(&root).expect("list").count(), 0);
    }

    #[test]
    fn the_shim_runs_the_pinned_bazel_with_the_startup_options() {
        let key = std::env::var("DX_TEST_NESTED_BAZEL_SHIM").expect("DX_TEST_NESTED_BAZEL_SHIM");
        let shim = resolve_runfile("DX_TEST_NESTED_BAZEL_SHIM", &key).expect("shim");
        let dir = scratch("nested-shim-");
        let (bazel, log) = recorder(dir.path());
        let shimmed = Config {
            shim: Some(shim.clone()),
            ..config(&dir.path().join("base"), &bazel)
        };
        let mut server = Server::acquire(shimmed).expect("slot");
        assert_eq!(
            std::fs::read(server.wrapper()).expect("installed"),
            std::fs::read(&shim).expect("shim")
        );
        let root = server.enter("ws").expect("enter");
        let run = capture(
            server.bazel_command(&root, &["info", "release"]),
            "bazel",
            true,
            &root,
        );
        assert!(run.status.success(), "{}", run.combined());
        assert_eq!(
            std::fs::read_to_string(&log).expect("log"),
            format!(
                "{}|unset|--output_user_root={} --max_idle_secs=5 --host_jvm_args=-Xmx1536m --bazelrc={} --nohome_rc --nosystem_rc info release\n",
                root.display(),
                server.user_root().display(),
                server.home().join("bazelrc").display()
            )
        );
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
    fn front_directories_go_ahead_of_the_wrapper_on_path() {
        let dir = scratch("nested-front-");
        let (bazel, _) = recorder(dir.path());
        let server = Server::acquire(config(&dir.path().join("base"), &bazel)).expect("slot");
        let shim = dir.path().join("shim");
        let command = server
            .command_with_front(Path::new("/usr/bin/env"), &[("PATH", "/custom")], &[&shim])
            .expect("command");
        let run = capture(command, "env", false, dir.path());
        let wrapper_dir = server.home().join("bin");
        let want = format!("PATH={}:{}:/custom", shim.display(), wrapper_dir.display());
        assert!(
            run.stdout.lines().any(|line| line == want),
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

#[cfg(test)]
mod portable_tests {
    use super::*;

    #[test]
    fn find_bazel_finds_exe_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("base");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("mkdir");
        std::fs::write(bin.join("bazelisk.exe"), "").expect("write");
        let path = std::env::join_paths([&bin]).expect("path");
        assert_eq!(
            find_bazel(None, None, &path, &base).expect("found"),
            bin.join("bazelisk.exe")
        );
        std::fs::write(bin.join("bazel.exe"), "").expect("write");
        assert_eq!(
            find_bazel(None, None, &path, &base).expect("found"),
            bin.join("bazel.exe")
        );
    }

    #[test]
    fn home_falls_back_to_userprofile() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| OsString::from(value))
            }
        };
        assert_eq!(
            pick_home(&vars(&[("HOME", "/h"), ("USERPROFILE", "C:/u")])),
            Some(PathBuf::from("/h"))
        );
        assert_eq!(
            pick_home(&vars(&[("HOME", ""), ("USERPROFILE", "C:/u")])),
            Some(PathBuf::from("C:/u"))
        );
        assert_eq!(
            pick_home(&vars(&[("USERPROFILE", "C:/u")])),
            Some(PathBuf::from("C:/u"))
        );
        assert_eq!(pick_home(&vars(&[("USERPROFILE", "")])), None);
    }

    #[test]
    fn repository_cache_scans_the_windows_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        assert_eq!(default_repository_cache(home), None);
        let windows = home.join("_bazel_me/cache/repos/v1");
        std::fs::create_dir_all(&windows).expect("mkdir");
        assert_eq!(default_repository_cache(home), Some(windows));
        let unix = home.join(".cache/bazel/_bazel_me/cache/repos/v1");
        std::fs::create_dir_all(&unix).expect("mkdir");
        assert_eq!(default_repository_cache(home), Some(unix));
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn shim_argv_puts_the_startup_flags_before_the_forwarded_args() {
        let text = "/r/bazel\r\n--output_user_root=/b/0/u\n--max_idle_secs=5\n";
        let (program, args) = shim_argv(text, &os(&["--nohome_rc", "info", "a b"])).expect("argv");
        assert_eq!(program, OsString::from("/r/bazel"));
        assert_eq!(
            args,
            os(&[
                "--output_user_root=/b/0/u",
                "--max_idle_secs=5",
                "--nohome_rc",
                "info",
                "a b"
            ])
        );
        assert_eq!(shim_argv("", &[]), None);
        assert_eq!(shim_argv("\n--x\n", &[]), None);
    }

    #[test]
    fn shim_args_round_trip_and_reject_newlines() {
        let config = Config {
            base: PathBuf::from("/b"),
            slots: 1,
            bazel: PathBuf::from("/r/bazel"),
            heap: DEFAULT_HEAP.to_owned(),
            disk_cache: PathBuf::from("/d"),
            repository_cache: None,
            offline: true,
            shim: None,
        };
        let argv = wrapper_argv(&config, Path::new("/b/0/u"), Path::new("/b/0/bazelrc"));
        let text = shim_args(&argv).expect("args");
        let (program, args) = shim_argv(&text, &os(&["build"])).expect("argv");
        let mut back = vec![program];
        back.extend(args);
        let mut want: Vec<OsString> = argv.iter().map(OsString::from).collect();
        want.push(OsString::from("build"));
        assert_eq!(back, want);
        let error =
            shim_args(&["/r/bazel".to_owned(), "--bazelrc=/a\nb".to_owned()]).expect_err("newline");
        assert!(error.contains("line break"), "{error}");
    }

    #[test]
    fn install_shim_replaces_a_read_only_copy_and_skips_an_identical_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("shim");
        std::fs::write(&src, "new").expect("write");
        let mut readonly = std::fs::metadata(&src).expect("meta").permissions();
        readonly.set_readonly(true);
        std::fs::set_permissions(&src, readonly.clone()).expect("chmod");
        let dst = dir.path().join("bin").join("bazel");
        std::fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
        std::fs::write(&dst, "old").expect("write");
        std::fs::set_permissions(&dst, readonly).expect("chmod");
        std::fs::write(old_shim(&dst, 1), "stale").expect("write");
        install_shim(&src, &dst).expect("install");
        assert_eq!(std::fs::read_to_string(&dst).expect("read"), "new");
        assert!(!std::fs::metadata(&dst)
            .expect("meta")
            .permissions()
            .readonly());
        assert!(!old_shim(&dst, 1).exists());
        let before = std::fs::metadata(&dst)
            .expect("meta")
            .modified()
            .expect("mtime");
        install_shim(&src, &dst).expect("again");
        let after = std::fs::metadata(&dst)
            .expect("meta")
            .modified()
            .expect("mtime");
        assert_eq!(before, after);
    }

    #[test]
    fn windows_process_lines_parse() {
        let text = "4242\tC:\\jdk\\java.exe --output_user_root=C:\\u --workspace_directory=C:\\w\r\nnoise\n\n17\t\"C:\\x.exe\" a\n";
        assert_eq!(
            parse_process_lines(text),
            vec![
                (
                    4242,
                    "C:\\jdk\\java.exe --output_user_root=C:\\u --workspace_directory=C:\\w"
                        .to_owned()
                ),
                (17, "\"C:\\x.exe\" a".to_owned()),
            ]
        );
    }

    #[test]
    fn workspace_match_accepts_backslashes_case_verbatim_prefix_and_quotes() {
        let flag = "--workspace_directory=";
        let line = "java.exe \"--workspace_directory=C:\\Users\\me\\.dxnb\\0\\w\\consumer\" --x";
        assert!(names_dir(line, flag, "C:/Users/me/.dxnb/0/w"));
        assert!(names_dir(line, flag, r"\\?\C:\Users\me\.dxnb\0\w"));
        assert!(names_dir(line, flag, "C:/Users/me/.dxnb/0/w/consumer"));
        assert_eq!(
            names_dir(line, flag, "c:/users/ME/.dxnb/0/w"),
            cfg!(windows)
        );
        assert!(names_dir(
            "java\0--output_user_root=/b/0/u\0x",
            "",
            "/b/0/u"
        ));
        assert!(names_dir("--workspace_directory=/a/ws", flag, "/a/ws/"));
    }

    #[test]
    fn workspace_match_rejects_a_sibling_prefix() {
        let flag = "--workspace_directory=";
        assert!(!names_dir(
            "java --workspace_directory=/a/ws2",
            flag,
            "/a/ws"
        ));
        assert!(!names_dir(
            "java --workspace_directory=/a/ws2/x",
            flag,
            "/a/ws"
        ));
        assert!(!names_dir(
            "java --x--workspace_directory=/a/ws",
            flag,
            "/a/ws"
        ));
        assert!(!names_dir("java --output_user_root=/b/0/u2", "", "/b/0/u"));
        assert!(!names_dir("java --workspace_directory=/a/ws", flag, ""));
        assert!(names_dir(
            "java --workspace_directory=/a/ws2 --workspace_directory=/a/ws/c",
            flag,
            "/a/ws"
        ));
    }

    #[cfg(windows)]
    #[test]
    fn alive_sees_this_process_and_not_a_dead_child() {
        assert!(alive(std::process::id()));
        let mut child = Command::new("cmd")
            .args(["/C", "exit 259"])
            .spawn()
            .expect("spawn");
        let pid = child.id();
        child.wait().expect("wait");
        assert!(!alive(pid));
    }
}
