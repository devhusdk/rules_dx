//! Runs nested Bazel servers for tests: one server under `TEST_TMPDIR`, hermetic environments and the fake Bazel.

pub mod cache;
pub mod compose;
pub mod footprint;
pub mod record;

use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::time::{Duration, Instant};

use dx_testing::Run;

pub const MAX_IDLE_SECS: u32 = 5;
pub const DEFAULT_HEAP: &str = "1536m";
pub const PINNED_ENV: &str = "DX_NESTED_BAZEL_PINNED";
pub const SHIM_ENV: &str = "DX_NESTED_BAZEL_SHIM";
pub const CPUS: u32 = 2;
/// The variable naming the cache set of the test; with the network flag it turns on record mode.
pub const CACHE_SET_ENV: &str = "DX_NESTED_CACHE_SET";
/// The variable naming the runfiles keys of the checked-in consumer locks, space separated.
pub const LOCKS_ENV: &str = "DX_NESTED_LOCKS";
const OUTPUTS_ENV: &str = "TEST_UNDECLARED_OUTPUTS_DIR";
const RECORD_LOGS: &str = "record-logs";
/// The variable naming the test's private temp directory; everything the harness writes is below it.
pub const TMPDIR_ENV: &str = "TEST_TMPDIR";
/// The `PATH` every nested command gets after the harness `bin` directory.
pub const NO_PATH: &str = "/dx_no_path";
const USER_ROOT: &str = "u";
const WORKSPACES: &str = "w";
const HOME_DIR: &str = "home";
const REPOSITORY_CACHE: &str = "repository-cache";

/// Environment variable naming the runfiles key of the pinned consumer Maven lock.
pub const CONSUMER_MAVEN_LOCK_ENV: &str = "DX_CONSUMER_MAVEN_LOCK";

/// The `maven` extension block every nested consumer module uses.
///
/// Pins resolution to `tools/testing/fixtures/consumer_maven/maven_install.json`, which
/// `stage_consumer_maven_lock` copies to `//:maven_install.json` of the nested workspace.
pub const CONSUMER_MAVEN: &str = r#"maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:32.0.1-jre"],
    fail_if_repin_required = True,
    known_contributing_modules = ["protobuf", "rules_dx"],
    lock_file = "//:maven_install.json",
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven")
"#;

/// Copies the pinned consumer Maven lock into the nested workspace at `root`.
pub fn stage_consumer_maven_lock(root: &Path) {
    let key = std::env::var(CONSUMER_MAVEN_LOCK_ENV)
        .unwrap_or_else(|_| panic!("{CONSUMER_MAVEN_LOCK_ENV} is not set; add CONSUMER_MAVEN_ENV"));
    let lock =
        dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("rlocation {key}: {error}"));
    let text = dx_testing::read(&lock);
    dx_testing::write(&root.join("maven_install.json"), &text);
}
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
    /// The directory under `TEST_TMPDIR` holding the home, the output user root and the workspaces.
    pub root: PathBuf,
    pub bazel: PathBuf,
    pub heap: String,
    pub offline: bool,
    /// The native `bazel` shim installed in the root; without it Unix falls back to a sh wrapper.
    pub shim: Option<PathBuf>,
    /// The declared downloads, as sha256 and file, that make up the repository cache.
    pub cache: Vec<(String, PathBuf)>,
    /// Record mode: the directory that receives the cache set and the locks of this run.
    pub record: Option<PathBuf>,
}

impl Config {
    /// Reads the settings from the test environment.
    pub fn from_env() -> Result<Config, String> {
        Self::from_parts(&|name| std::env::var_os(name), &resolve_runfile)
    }

    /// Builds the settings from an environment lookup and a runfiles resolver.
    pub fn from_parts(
        lookup: &dyn Fn(&str) -> Option<OsString>,
        resolve: &dyn Fn(&str, &str) -> Result<PathBuf, String>,
    ) -> Result<Config, String> {
        let text = |name: &str| lookup(name).and_then(|v| v.to_str().map(str::to_owned));
        let root = env_path(lookup, TMPDIR_ENV)
            .ok_or_else(|| format!("{TMPDIR_ENV} is not set; a nested Bazel runs only inside a test"))?
            .join("nested");
        let heap = text("DX_NESTED_BAZEL_HEAP")
            .filter(|heap| !heap.is_empty())
            .unwrap_or_else(|| DEFAULT_HEAP.to_owned());
        let pinned = text(PINNED_ENV)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| {
                format!("{PINNED_ENV} is not set; declare the test with nested_bazel_test")
            })?;
        let bazel = pinned_bazel(resolve(PINNED_ENV, &pinned)?)?;
        let shim = match text(SHIM_ENV).filter(|key| !key.is_empty()) {
            Some(key) => Some(resolve(SHIM_ENV, &key)?),
            None => None,
        };
        let mut cache = Vec::new();
        for key in text(cache::MANIFESTS_ENV).unwrap_or_default().split_whitespace() {
            let manifest = resolve(cache::MANIFESTS_ENV, key)?;
            let listed = std::fs::read_to_string(&manifest)
                .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
            for (sha256, file) in cache::parse_manifest(&listed)? {
                cache.push((sha256, resolve(cache::MANIFESTS_ENV, &file)?));
            }
        }
        let offline = lookup("DX_NESTED_BAZEL_NETWORK").is_none_or(|v| v.is_empty());
        let record = if offline || text(CACHE_SET_ENV).is_none_or(|set| set.is_empty()) {
            None
        } else {
            let outputs = env_path(lookup, OUTPUTS_ENV).ok_or_else(|| {
                format!("{OUTPUTS_ENV} is not set; record mode runs only inside a test")
            })?;
            if shim.is_none() {
                return Err(format!("{SHIM_ENV} is not set; record mode needs the native shim"));
            }
            Some(outputs.join("nested_cache"))
        };
        Ok(Config {
            root,
            bazel,
            heap,
            offline,
            shim,
            cache,
            record,
        })
    }
}

/// Resolves the runfiles key in the variable `name` to an absolute path.
pub fn resolve_runfile(name: &str, key: &str) -> Result<PathBuf, String> {
    let found = dx_testing::rlocation(key)
        .map_err(|error| format!("{name} {key} is not in the runfiles: {error}"))?;
    std::path::absolute(&found).map_err(|error| format!("{name} {}: {error}", found.display()))
}

fn env_path(lookup: &dyn Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Returns the pinned binary `pinned`, or an error when it is not a file.
pub fn pinned_bazel(pinned: PathBuf) -> Result<PathBuf, String> {
    if pinned.is_file() {
        return Ok(pinned);
    }
    Err(format!("{PINNED_ENV} {} is not a file", pinned.display()))
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

/// The real Bazel followed by every pinned nested startup option; the shim sidecar's contents.
pub fn wrapper_argv(config: &Config, user_root: &Path, rc: &Path) -> Vec<String> {
    vec![
        config.bazel.to_string_lossy().into_owned(),
        format!("--output_user_root={}", user_root.display()),
        format!("--max_idle_secs={MAX_IDLE_SECS}"),
        format!("--host_jvm_args=-Xmx{}", config.heap),
        format!("--bazelrc={}", rc.display()),
    ]
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
    if let Some(owner) = std::env::var_os(REAP_ENV) {
        let workspaces = std::env::args_os().nth(1).map(PathBuf::from);
        return match workspaces {
            Some(workspaces) => reap(&owner.to_string_lossy(), &workspaces),
            None => 2,
        };
    }
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
    if let Ok(record) = std::fs::read_to_string(me.with_extension("record"))
        && let Err(error) = write_record_rc(&record)
    {
        eprintln!("nested bazel shim: {error}");
        return 2;
    }
    let mut command = Command::new(&program);
    command.args(&args);
    run_launcher(command, &program)
}

/// Writes the rc file of one call from a `.record` sidecar: the rc path, then the log directory.
fn write_record_rc(record: &str) -> Result<(), String> {
    let mut lines = record.lines();
    let (Some(rc), Some(logs)) = (lines.next(), lines.next()) else {
        return Err("the .record sidecar needs an rc path and a log directory".to_owned());
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    std::fs::create_dir_all(logs).map_err(|error| format!("cannot create {logs}: {error}"))?;
    let log = Path::new(logs).join(format!("{}-{nanos}.log", std::process::id()));
    write(
        Path::new(rc),
        &format!(
            "common --experimental_workspace_rules_log_file={}\n",
            slashed(&log)
        ),
    )
}

#[cfg(unix)]
fn run_launcher(mut command: Command, program: &std::ffi::OsStr) -> i32 {
    use std::os::unix::process::CommandExt;
    let error = command.exec();
    eprintln!(
        "nested bazel shim: cannot run {}: {error}",
        program.to_string_lossy()
    );
    127
}

#[cfg(not(unix))]
fn run_launcher(mut command: Command, program: &std::ffi::OsStr) -> i32 {
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
pub fn bazelrc(config: &Config, downloader: &Path, repository_cache: &Path) -> String {
    let mut text = format!("common --local_resources=cpu={CPUS}\n");
    if config.offline {
        text.push_str(&format!(
            "common --downloader_config={}\n",
            slashed(downloader)
        ));
        text.push_str("common --lockfile_mode=error\n");
    }
    if config.record.is_some() {
        text.push_str("common --lockfile_mode=update\n");
        text.push_str(&format!(
            "try-import {}\n",
            slashed(&record_rc(&config.root))
        ));
    }
    text.push_str("common --disk_cache=\n");
    text.push_str("common --remote_cache=\n");
    text.push_str(&format!(
        "common --repository_cache={}\n",
        slashed(repository_cache)
    ));
    text.push_str("common --repo_env=BAZEL_HTTP_RULES_URLS_AS_DEFAULT_CANONICAL_ID=0\n");
    text.push_str("build --spawn_strategy=local\n");
    text
}

fn record_rc(home: &Path) -> PathBuf {
    home.join("record.rc")
}

/// Bazel reads a backslash in a bazelrc as an escape, so Windows paths use slashes.
fn slashed(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
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

    #[cfg(target_os = "linux")]
    const STOP: i32 = 19;
    #[cfg(target_os = "linux")]
    const CONT: i32 = 18;
    #[cfg(not(target_os = "linux"))]
    const STOP: i32 = 17;
    #[cfg(not(target_os = "linux"))]
    const CONT: i32 = 19;

    pub fn suspend(pid: u32) -> bool {
        send(pid, STOP)
    }

    pub fn resume(pid: u32) -> bool {
        send(pid, CONT)
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
    const PROCESS_SUSPEND_RESUME: u32 = 0x0800;
    const WAIT_TIMEOUT: u32 = 0x0000_0102;

    #[link(name = "ntdll", kind = "raw-dylib")]
    unsafe extern "system" {
        fn NtSuspendProcess(process: Handle) -> i32;
        fn NtResumeProcess(process: Handle) -> i32;
    }

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

    fn with_handle(pid: u32, act: unsafe extern "system" fn(Handle) -> i32) -> bool {
        let handle = unsafe { OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let status = unsafe { act(handle) };
        unsafe {
            CloseHandle(handle);
        }
        status >= 0
    }

    pub fn suspend(pid: u32) -> bool {
        with_handle(pid, NtSuspendProcess)
    }

    pub fn resume(pid: u32) -> bool {
        with_handle(pid, NtResumeProcess)
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

    pub fn suspend(_pid: u32) -> bool {
        false
    }

    pub fn resume(_pid: u32) -> bool {
        false
    }
}

/// A process frozen by `suspend`; it is resumed on drop.
pub struct Suspended {
    pid: u32,
    held: bool,
}

/// Freezes every thread of `pid` until the guard resumes or drops.
pub fn suspend(pid: u32) -> Result<Suspended, String> {
    if signal::suspend(pid) {
        Ok(Suspended { pid, held: true })
    } else {
        Err(format!("cannot suspend process {pid}"))
    }
}

impl Suspended {
    /// Resumes the process now; later calls and the drop do nothing.
    pub fn resume(&mut self) -> bool {
        let done = self.held && signal::resume(self.pid);
        self.held = false;
        done
    }
}

impl Drop for Suspended {
    fn drop(&mut self) {
        self.resume();
    }
}

/// Reports whether `pid` is a live process.
pub fn alive(pid: u32) -> bool {
    #[cfg(windows)]
    return signal::alive(pid);
    #[cfg(not(windows))]
    alive_unix(pid)
}

#[cfg(not(windows))]
fn alive_unix(pid: u32) -> bool {
    if !signal::alive(pid) {
        return false;
    }
    if cfg!(windows) {
        return true;
    }
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
        Err(_) if Path::new("/proc/self").exists() => true,
        Err(_) => dx_process::probe("ps", ["-o", "stat=", "-p", &pid.to_string()])
            .is_none_or(|stat| !stat.trim_start().starts_with('Z')),
    }
}

#[cfg(unix)]
fn command_line(pid: u32) -> Option<String> {
    if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) {
        return Some(String::from_utf8_lossy(&cmdline).into_owned());
    }
    None
}

#[cfg(unix)]
fn all_pids() -> Vec<u32> {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        return entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
            .collect();
    }
    Vec::new()
}

/// Parses `ps -o pid=,stat=,command=` lines into `(pid, state, command line)`, skipping junk.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_ps_lines(text: &str) -> Vec<(u32, char, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, rest) = line.split_once(char::is_whitespace)?;
            let pid = pid.parse::<u32>().ok()?;
            let rest = rest.trim_start();
            let (stat, command) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some((pid, stat.chars().next()?, command.trim().to_owned()))
        })
        .collect()
}

/// One sweep's view of the process table: Unix reads `/proc` per pid, Windows takes one snapshot.
struct Processes {
    #[cfg(not(unix))]
    table: Vec<(u32, String)>,
    /// `(pid, state, command line)` from one `ps` call; `None` when `/proc` is available.
    #[cfg(unix)]
    ps: Option<Vec<(u32, char, String)>>,
}

impl Processes {
    #[cfg(unix)]
    fn snapshot() -> Processes {
        if Path::new("/proc/self").exists() {
            return Processes { ps: None };
        }
        Processes::from_ps()
    }

    /// Takes one `ps` listing of every process, whatever the host offers.
    #[cfg(unix)]
    fn from_ps() -> Processes {
        let text =
            dx_process::probe("ps", ["-A", "-ww", "-o", "pid=,stat=,command="]).unwrap_or_default();
        Processes {
            ps: Some(parse_ps_lines(&text)),
        }
    }

    /// Reports whether `pid` is a live, non-zombie process in this view.
    #[cfg(unix)]
    fn alive(&self, pid: u32) -> bool {
        match &self.ps {
            Some(table) => {
                signal::alive(pid)
                    && table
                        .iter()
                        .find(|(each, _, _)| *each == pid)
                        .is_none_or(|(_, state, _)| *state != 'Z')
            }
            None => alive(pid),
        }
    }

    #[cfg(not(unix))]
    fn alive(&self, pid: u32) -> bool {
        alive(pid)
    }

    #[cfg(windows)]
    fn snapshot() -> Processes {
        Processes {
            table: dx_process::procs::command_lines()
                .into_iter()
                .map(|info| (info.pid, info.command_line))
                .collect(),
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn snapshot() -> Processes {
        Processes { table: Vec::new() }
    }

    #[cfg(unix)]
    fn command_line(&self, pid: u32) -> Option<String> {
        match &self.ps {
            Some(table) => table
                .iter()
                .find(|(each, _, _)| *each == pid)
                .map(|(_, _, line)| line.clone()),
            None => command_line(pid),
        }
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
        match &self.ps {
            Some(table) => table.iter().map(|(pid, _, _)| *pid).collect(),
            None => all_pids(),
        }
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

#[cfg(test)]
#[cfg_attr(not(unix), allow(dead_code))]
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
        .filter(|pid| processes.alive(*pid) && owned_in(processes, *pid, user_root))
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
        .filter(|pid| *pid != own && processes.alive(*pid))
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

/// One nested Bazel server: its root under `TEST_TMPDIR`, its output user root and its workspace.
pub struct Server {
    config: Config,
    home: PathBuf,
    workspace: Option<PathBuf>,
    reaper: Option<Child>,
}

impl Server {
    /// Lays out the configured root: the wrapper, the rc file and the repository cache.
    pub fn acquire(config: Config) -> Result<Server, String> {
        let home = config.root.clone();
        let mut server = Server {
            config,
            home,
            workspace: None,
            reaper: None,
        };
        for dir in [server.home.join(WORKSPACES), server.home.join(HOME_DIR)] {
            std::fs::create_dir_all(&dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        }
        kill_slot_servers(&server.user_root(), &server.home.join(WORKSPACES));
        let rc = server.home.join("bazelrc");
        let downloader = server.home.join("downloader.cfg");
        let repository_cache = server.repository_cache();
        if server.config.record.is_none() {
            cache::install(&repository_cache, &server.config.cache)?;
        }
        write(&downloader, DOWNLOADER)?;
        write(&rc, &bazelrc(&server.config, &downloader, &repository_cache))?;
        let launcher = server.config.shim.clone().ok_or_else(|| {
            format!(
                "{SHIM_ENV} is not set; nested Bazel needs the native shim (add NESTED_BAZEL_DATA and NESTED_BAZEL_ENV)"
            )
        })?;
        let wrapper = server.wrapper();
        let argv = wrapper_argv(&server.config, &server.user_root(), &rc);
        install_shim(&launcher, &wrapper)?;
        write(&wrapper.with_extension("args"), &shim_args(&argv)?)?;
        if server.config.record.is_some() {
            let logs = server.home.join(RECORD_LOGS);
            let rc = record_rc(&server.home);
            write(&rc, "")?;
            write(
                &wrapper.with_extension("record"),
                &format!("{}\n{}\n", rc.display(), logs.display()),
            )?;
        }
        server.reaper = spawn_reaper(&wrapper, std::process::id(), &server.home);
        Ok(server)
    }

    /// The repository cache built from the declared downloads.
    pub fn repository_cache(&self) -> PathBuf {
        self.home.join(REPOSITORY_CACHE)
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn user_root(&self) -> PathBuf {
        self.home.join(USER_ROOT)
    }

    pub fn wrapper(&self) -> PathBuf {
        wrapper_at(&self.home)
    }

    /// Empties the workspace `name` and makes it current, stopping the server of any other one.
    pub fn enter(&mut self, name: &str) -> Result<PathBuf, String> {
        let root = self.home.join(WORKSPACES).join(name);
        if self.workspace.as_ref() != Some(&root) {
            self.shutdown();
            self.workspace = Some(root.clone());
        }
        reset(&root)?;
        Ok(root)
    }

    /// Lists the live servers this slot owns.
    pub fn servers(&self) -> Vec<u32> {
        slot_servers(&self.user_root(), &self.home.join(WORKSPACES))
    }

    /// Stops the server and returns any that survive.
    pub fn shutdown(&mut self) -> Vec<u32> {
        if let Some(root) = self.workspace.as_ref().filter(|root| root.is_dir())
            && !self.servers().is_empty()
        {
            let _ = self.bazel_command(root, &["shutdown"]).output();
        }
        kill_slot_servers(&self.user_root(), &self.home.join(WORKSPACES))
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
    let user_home = home.join(HOME_DIR);
    command.env("HOME", &user_home);
    command.env("USERPROFILE", &user_home);
    let mut path = None;
    for (key, value) in envs {
        if *key == "PATH" {
            path = Some(OsString::from(value));
        } else {
            command.env(key, value);
        }
    }
    let path = path.unwrap_or_else(|| OsString::from(NO_PATH));
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

/// Names the environment variable that makes the shim watch an owner pid instead of running Bazel.
const REAP_ENV: &str = "DX_NESTED_REAP";

/// Names the file in the slot home that receives the reaper's stderr.
const REAPER_LOG: &str = "reaper.log";

/// Starts a detached shim that sweeps the slot's workspaces once `owner` is gone, however it died.
#[cfg(unix)]
fn spawn_reaper(launcher: &Path, owner: u32, home: &Path) -> Option<Child> {
    use std::os::unix::process::CommandExt;
    let workspaces = home.join(WORKSPACES);
    Command::new(launcher)
        .env(REAP_ENV, owner.to_string())
        .arg(&workspaces)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(reaper_log(home))
        .process_group(0)
        .spawn()
        .ok()
}

#[cfg(windows)]
fn spawn_reaper(launcher: &Path, owner: u32, home: &Path) -> Option<Child> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let workspaces = home.join(WORKSPACES);
    Command::new(launcher)
        .env(REAP_ENV, owner.to_string())
        .arg(&workspaces)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(reaper_log(home))
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .ok()
}

/// Opens the reaper's stderr log in the slot home, truncated on each acquire.
#[cfg(any(unix, windows))]
fn reaper_log(home: &Path) -> std::process::Stdio {
    std::fs::File::create(home.join(REAPER_LOG))
        .map_or_else(|_| std::process::Stdio::null(), Into::into)
}

#[cfg(not(any(unix, windows)))]
fn spawn_reaper(_launcher: &Path, _owner: u32, _home: &Path) -> Option<Child> {
    None
}

/// The reaper: waits for `owner` to exit, then stops every server under `workspaces`.
fn reap(owner: &str, workspaces: &Path) -> i32 {
    let Ok(owner) = owner.parse::<u32>() else {
        eprintln!("nested bazel shim: {REAP_ENV} is not a pid");
        return 2;
    };
    while alive(owner) {
        std::thread::sleep(Duration::from_secs(1));
    }
    let real = std::fs::canonicalize(workspaces).unwrap_or_else(|_| workspaces.to_path_buf());
    eprintln!(
        "nested bazel reaper: owner {owner} is gone; sweeping {} and {}",
        workspaces.display(),
        real.display()
    );
    for root in [workspaces, real.as_path()] {
        let seen = servers_in_workspaces(root);
        let left = kill_servers_in_workspaces(root);
        eprintln!(
            "nested bazel reaper: {}: saw {seen:?}, survivors {left:?}",
            root.display()
        );
    }
    0
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

const DOWNLOADER: &str = "block *\n";
const STALE_LOCK: &str = "--lockfile_mode=update";
const BLOCKED: &str = "Configured URL rewriter blocked all URLs:";

/// Explains a failed offline run whose download the nested downloader blocked.
fn blocked_download(offline: bool, run: &Run) -> Option<String> {
    if !offline || run.status.success() {
        return None;
    }
    let combined = run.combined();
    if let Some(line) = combined.lines().find(|line| line.contains(STALE_LOCK)) {
        return Some(format!(
            "{}\nthe checked-in consumer lock is stale; regenerate it with `bazel run //tools/testing:nested_cache_update -- <this test's label>`",
            line.trim()
        ));
    }
    let line = combined.lines().find(|line| line.contains(BLOCKED))?;
    Some(format!(
        "{}\nthe URL is not in the test's nested_repository_cache sets; add the set or regenerate its list with `bazel run //tools/testing:nested_cache_update`",
        line.trim()
    ))
}

/// The longest one captured command may run.
const CALL_DEADLINE: Duration = Duration::from_secs(300);

/// Like `Command::output`, but kills the child and returns an error naming the command and its output so far once `limit` passes.
fn output_within(command: &mut Command, limit: Duration) -> Result<std::process::Output, String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    fn drain<R: Read + Send + 'static>(stream: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut stream) = stream {
                let _ = stream.read_to_end(&mut bytes);
            }
            bytes
        })
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if start.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                let tail = |bytes: Vec<u8>| {
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    let keep = text.len().saturating_sub(4000);
                    text[text.ceil_char_boundary(keep)..].to_string()
                };
                let out = tail(stdout.join().unwrap_or_default());
                let err = tail(stderr.join().unwrap_or_default());
                return Err(format!(
                    "{command:?} did not finish within {}s and was killed\nstdout (tail):\n{out}\nstderr (tail):\n{err}",
                    limit.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    Ok(std::process::Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

fn capture(mut command: Command, what: &str, offline: bool) -> Run {
    let output = output_within(&mut command, CALL_DEADLINE)
        .unwrap_or_else(|error| panic!("run {what}: {error}"));
    let run = Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    if let Some(message) = blocked_download(offline, &run) {
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
    if let Some(server) = &taken
        && let Err(error) = finish_record(server)
    {
        eprintln!("nested bazel: record mode: {error}");
        std::process::abort();
    }
    drop(taken);
}

static RECORDED: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

/// True when the test runs online with a cache set: the run records the set and the locks.
pub fn recording() -> bool {
    let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    set("DX_NESTED_BAZEL_NETWORK") && set(CACHE_SET_ENV)
}

fn read_dir_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(|entry| entry.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.sort();
    files
}

/// Writes the cache set and the locks of a record mode run into the record directory.
fn finish_record(server: &Server) -> Result<(), String> {
    let Some(dir) = server.config.record.as_ref() else {
        return Ok(());
    };
    let set = std::env::var(CACHE_SET_ENV).map_err(|error| format!("{CACHE_SET_ENV}: {error}"))?;
    let roots = std::mem::take(&mut *RECORDED.lock().unwrap_or_else(PoisonError::into_inner));
    if roots.is_empty() {
        return Ok(());
    }
    let repository_cache = server.repository_cache();
    let fetched = |sha256: &str| std::fs::read(cache::entry(&repository_cache, sha256)).ok();
    let mut entries: std::collections::BTreeMap<String, String> = Default::default();
    let mut locks: std::collections::BTreeMap<String, String> = Default::default();
    for (stem, root) in &roots {
        let path = root.join("MODULE.bazel.lock");
        let lock = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if locks.get(stem).is_some_and(|seen| *seen != lock) {
            return Err(format!("the lock {stem} was recorded twice with different content"));
        }
        let module_path = root.join("MODULE.bazel");
        let module = std::fs::read_to_string(&module_path)
            .map_err(|error| format!("cannot read {}: {error}", module_path.display()))?;
        for line in cache::list(&lock, &module, &fetched)?.lines() {
            if let Some((url, sha256)) = line.split_once(' ') {
                entries.insert(url.to_owned(), sha256.to_owned());
            }
        }
        locks.insert(stem.clone(), lock);
    }
    for log in read_dir_files(&server.home.join(RECORD_LOGS)) {
        let bytes = std::fs::read(&log)
            .map_err(|error| format!("cannot read {}: {error}", log.display()))?;
        for (url, sha256) in record::downloads(&bytes) {
            entries.insert(url, sha256);
        }
    }
    for (stem, lock) in &locks {
        write(&dir.join(format!("{stem}.MODULE.bazel.lock")), lock)?;
    }
    let mut list = String::new();
    for (url, sha256) in &entries {
        list.push_str(&format!("{url} {sha256}\n"));
    }
    write(&dir.join(format!("{set}.txt")), &list)
}

type Lease = RefCell<MutexGuard<'static, Option<Server>>>;

thread_local! {
    static HELD: RefCell<Weak<Lease>> = const { RefCell::new(Weak::new()) };
}

static HOME: Mutex<Option<(PathBuf, bool)>> = Mutex::new(None);
static CONFIG: Mutex<Option<Config>> = Mutex::new(None);

/// Sets the settings the shared server of this process starts with, in place of the test environment.
pub fn configure(config: Config) {
    *CONFIG.lock().unwrap_or_else(PoisonError::into_inner) = Some(config);
}

/// The query the harness consumer answers offline, and the cache update tool runs online.
pub const HARNESS_QUERY: &[&str] = &[
    "query",
    "--color=no",
    "--curses=no",
    "@@rules_py++python_interpreters+python_interpreters//:all",
];

/// Starts the consumer of the harness self-test: rules_dx from the `module_srcs` runfiles, with
/// the rules_py override and its patches.
pub fn harness_consumer(name: &str) -> Case {
    let module = dx_testing::resolve_runfiles("MODULE.bazel");
    let module = std::path::absolute(&module).unwrap_or_else(|error| panic!("{module:?}: {error}"));
    let rules = module
        .parent()
        .unwrap_or_else(|| panic!("no directory above {module:?}"));
    let case = Case::consumer_at(name, rules, &compose::rules_py_override());
    case.write("patches/rules_py/BUILD.bazel", "");
    for patch in compose::RULES_PY_PATCHES {
        let rel = format!("patches/rules_py/{patch}.patch");
        case.write(&rel, &dx_testing::read_text(&rules.join(&rel)));
    }
    case
}

/// Shared settings of `@rules_dx//config` that dx passes to Bazel, as `(name, default)`.
///
/// Source of truth: `BAZEL_SETTINGS` in `cli/process/src/lib.rs`; `validate` is kept because
/// users may pass `--@rules_dx//config:validate=false` through. A new shared setting is one
/// entry here.
pub const RULES_DX_FLAGS: &[(&str, bool)] = &[
    ("validate", true),
    ("clippy_output_diagnostics", false),
    ("rustc_output_diagnostics", false),
];

/// `MODULE.bazel` text for a fixture whose root module is visible as `@rules_dx`.
///
/// `attrs` is appended inside `module(...)` (for example `, version = "1"`); `body` follows it.
pub fn rules_dx_module(attrs: &str, body: &str) -> String {
    format!("module(name = \"fixture\", repo_name = \"rules_dx\"{attrs})\n{body}")
}

/// Lock keys a seeded consumer lock keeps from the rules_dx lock; every other top-level key is emptied.
const SEED_LOCK_KEPT: &[&str] = &["lockFileVersion", "facts", "factsVersions"];

/// Turns the rules_dx `MODULE.bazel.lock` text into a consumer lock that carries only its extension facts.
///
/// Facts hold what extensions downloaded without a checksum (for example the python_interpreters
/// SHA256SUMS), so a seeded consumer resolves offline. Bazel keeps facts across extension changes.
pub fn seed_lock_json(lock: &str) -> Result<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(lock).map_err(|error| format!("MODULE.bazel.lock: {error}"))?;
    let serde_json::Value::Object(map) = parsed else {
        return Err("MODULE.bazel.lock: not a JSON object".to_string());
    };
    if !map.contains_key("lockFileVersion") {
        return Err("MODULE.bazel.lock: no lockFileVersion".to_string());
    }
    match map.get("facts") {
        Some(serde_json::Value::Object(facts)) if !facts.is_empty() => {}
        _ => return Err("MODULE.bazel.lock: no extension facts to seed".to_string()),
    }
    let mut seeded = serde_json::Map::new();
    for (key, value) in map {
        let value = if SEED_LOCK_KEPT.contains(&key.as_str()) {
            value
        } else if value.is_array() {
            serde_json::Value::Array(Vec::new())
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        seeded.insert(key, value);
    }
    let mut text = serde_json::to_string_pretty(&serde_json::Value::Object(seeded))
        .map_err(|error| format!("MODULE.bazel.lock: {error}"))?;
    text.push('\n');
    Ok(text)
}

/// Writes `root/MODULE.bazel.lock` holding the extension facts of the rules_dx checkout at `rules`.
///
/// The consumer must pin the `.bazelversion` of the same checkout, or Bazel ignores the lock.
pub fn seed_rules_dx_lock(root: &Path, rules: &Path) {
    let source = rules.join("MODULE.bazel.lock");
    let lock = std::fs::read_to_string(&source).unwrap_or_else(|error| {
        panic!("{source:?}: {error}; add //:module_srcs to the test's data")
    });
    let seeded = seed_lock_json(&lock).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    let target = root.join("MODULE.bazel.lock");
    std::fs::write(&target, seeded).unwrap_or_else(|error| panic!("write {target:?}: {error}"));
}

/// Writes the `config` package declaring [`RULES_DX_FLAGS`] under `root`.
pub fn write_rules_dx_config(root: &Path) {
    let dir = root.join("config");
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir {dir:?}: {error}"));
    let flags = "def _impl(_ctx):\n    return []\n\nbool_flag = rule(implementation = _impl, build_setting = config.bool(flag = True))\n";
    let mut build = String::from("load(\":flags.bzl\", \"bool_flag\")\n");
    for (name, default) in RULES_DX_FLAGS {
        let default = if *default { "True" } else { "False" };
        build.push_str(&format!(
            "\nbool_flag(name = \"{name}\", build_setting_default = {default})\n"
        ));
    }
    for (file, text) in [("flags.bzl", flags), ("BUILD.bazel", build.as_str())] {
        std::fs::write(dir.join(file), text)
            .unwrap_or_else(|error| panic!("write {file}: {error}"));
    }
}

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
            let configured = CONFIG.lock().unwrap_or_else(PoisonError::into_inner).take();
            let config = match configured {
                Some(config) => config,
                None => Config::from_env().unwrap_or_else(|error| panic!("nested bazel: {error}")),
            };
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
        offline)
}

impl Case {
    /// Puts the checked-in lock `stem` into the workspace; in record mode removes it and records the workspace.
    pub fn use_lock(&self, stem: &str) {
        if recording() {
            let _ = std::fs::remove_file(self.root.join("MODULE.bazel.lock"));
            let mut roots = RECORDED.lock().unwrap_or_else(PoisonError::into_inner);
            let entry = (stem.to_owned(), self.root.clone());
            if !roots.contains(&entry) {
                roots.push(entry);
            }
            return;
        }
        let name = format!("{stem}.MODULE.bazel.lock");
        let locks = std::env::var(LOCKS_ENV)
            .unwrap_or_else(|_| panic!("{LOCKS_ENV} is not set; declare the test with nested_bazel_test(cache = ...)"));
        for key in locks.split_whitespace() {
            let path = resolve_runfile(LOCKS_ENV, key).unwrap_or_else(|error| panic!("{error}"));
            if path.file_name().is_some_and(|file| *file == *name) {
                self.write("MODULE.bazel.lock", &dx_testing::read_text(&path));
                return;
            }
        }
        panic!("{LOCKS_ENV} has no {name}");
    }

    /// Seeds this workspace's `MODULE.bazel.lock` with the extension facts of the rules_dx checkout.
    pub fn seed_rules_dx_lock(&self) {
        seed_rules_dx_lock(&self.root, &dx_testing::rules_dx_source());
    }

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

    /// The repository cache the server reads, built from the declared downloads.
    pub fn repository_cache(&self) -> PathBuf {
        self.server().repository_cache()
    }

    pub fn servers(&self) -> Vec<u32> {
        self.server().servers()
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

    /// Stops the server, reruns `args` with the workspace rules log at `log` under `TEST_TMPDIR`, and returns the repository rules that ran,
    /// without the modules that the consumer's MODULE.bazel overrides with `local_path_override`.
    pub fn refetched_after_restart(&self, args: &[&str], log: &str) -> Vec<String> {
        self.shutdown_server();
        let tmpdir = PathBuf::from(std::env::var_os("TEST_TMPDIR").expect("TEST_TMPDIR"));
        let log = tmpdir.join(log);
        let _ = std::fs::remove_file(&log);
        let flag = format!("--experimental_workspace_rules_log_file={}", log.display());
        let mut full = args.to_vec();
        full.push(&flag);
        let run = self.bazel(&full);
        assert!(run.status.success(), "{}", run.combined());
        let bytes = std::fs::read(&log).unwrap_or_default();
        let local = compose::local_override_repos(&dx_testing::read_text(
            &self.root().join("MODULE.bazel"),
        ));
        record::contexts(&bytes)
            .into_iter()
            .filter(|context| !local.contains(context))
            .collect()
    }

    /// Stops the server, reruns `args`, and fails with the repository rules that ran.
    pub fn assert_no_refetch(&self, args: &[&str]) {
        let contexts = self.refetched_after_restart(args, "refetch.log");
        assert!(
            contexts.is_empty(),
            "a restarted server refetched repositories: {contexts:?}"
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
        capture(command, &bin.display().to_string(), offline)
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
        capture(command, &bin.display().to_string(), offline)
    }

    /// Runs `bazel` in the workspace with the startup options dx uses.
    pub fn bazel(&self, args: &[&str]) -> Run {
        bazel_in(&self.root, args)
    }

    /// Runs `bazel` in the workspace with extra environment variables.
    pub fn bazel_with(&self, args: &[&str], envs: &[(&str, &str)]) -> Run {
        let command = bazel_command_at(self.server().home(), &self.root, args, envs);
        let offline = self.server().config.offline;
        capture(command, "nested bazel", offline)
    }

    /// Builds an unspawned `bazel` command in the workspace with the startup options dx uses.
    pub fn bazel_command(&self, args: &[&str]) -> Command {
        bazel_command_at(self.server().home(), &self.root, args, &[])
    }

    /// Starts a case whose workspace depends on this rules_dx checkout, with `extra_module` appended.
    ///
    /// The workspace imports the preset layers from `.dx/bazelrc`, the way `dx init` lays them out.
    pub fn consumer(name: &str, extra_module: &str) -> Case {
        Case::consumer_at(name, &dx_testing::rules_dx_source(), extra_module)
    }

    /// Like `consumer`, with rules_dx read from `rules`.
    pub fn consumer_at(name: &str, rules: &Path, extra_module: &str) -> Case {
        let case = case(name);
        case.write(
            ".bazelversion",
            &dx_testing::read_text(&rules.join(".bazelversion")),
        );
        case.write("MODULE.bazel", &consumer_module(rules, extra_module));
        seed_rules_dx_lock(case.root(), rules);
        let mut imports = String::new();
        for layer in dx_preset::Layer::ALL {
            let file = layer.file_name();
            case.write(
                format!(".dx/bazelrc/{file}"),
                &dx_testing::preset_layer(layer),
            );
            imports.push_str(&format!("try-import %workspace%/.dx/bazelrc/{file}\n"));
        }
        case.write(".bazelrc", &imports);
        case.write(".bazelignore", ".dx\n");
        case
    }

    /// Writes `text` to `rel` under the workspace, creating its parent directories.
    pub fn write(&self, rel: impl AsRef<Path>, text: &str) {
        dx_testing::write_text(&self.root.join(rel), text);
    }

    /// Lists every workspace file with its bytes, sorted, without `bazel-*` links or the lockfile.
    pub fn files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        files_under(&self.root)
    }

    /// Runs `bazel <verb> --color=no --curses=no <flags> <target>` in the workspace.
    pub fn plain(&self, verb: &str, target: &str, flags: &[&str]) -> Run {
        let mut args = vec![verb, "--color=no", "--curses=no"];
        args.extend_from_slice(flags);
        args.push(target);
        self.bazel(&args)
    }

    /// Asserts that building `target` exits 1 with every one of `needles` in its output.
    pub fn build_fails_with(&self, target: &str, needles: &[&str]) {
        let built = self.plain("build", target, &[]);
        let text = built.combined();
        assert_eq!(built.status.code(), Some(1), "{text}");
        for needle in needles {
            assert!(text.contains(needle), "{needle}\n{text}");
        }
    }
}

fn consumer_module(rules: &Path, extra: &str) -> String {
    format!(
        "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n{}{extra}",
        compose::rules_dx_override(rules)
    )
}

fn files_under(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files);
    files
}

fn collect_files(dir: &Path, root: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("list {dir:?}: {error}"))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path()
        })
        .collect();
    entries.sort();
    for path in entries {
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let name = relative.to_string_lossy();
        if name.starts_with("bazel-") || name == "MODULE.bazel.lock" {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, root, files);
        } else {
            files.push((relative, std::fs::read(&path).unwrap_or_default()));
        }
    }
}

impl std::ops::Deref for Case {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
fn runfile(var: &str) -> PathBuf {
    let key = std::env::var(var).unwrap_or_else(|_| panic!("{var} is not set"));
    resolve_runfile(var, &key).unwrap_or_else(|error| panic!("{error}"))
}

#[cfg(test)]
fn probe() -> PathBuf {
    runfile("DX_TEST_NESTED_PROBE")
}

#[cfg(test)]
fn probe_command(args: &[&str]) -> Command {
    let mut command = Command::new(probe());
    command.args(args);
    command
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

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

    #[test]
    fn write_record_rc_creates_the_log_directory() {
        let dir = scratch("nested-record-");
        let root = dir.path().display();
        write_record_rc(&format!("{root}/rc\n{root}/missing/logs\n")).expect("record rc");
        assert!(dir.path().join("missing/logs").is_dir());
        let rc = std::fs::read_to_string(dir.path().join("rc")).expect("rc");
        assert!(rc.contains("--experimental_workspace_rules_log_file="), "{rc}");
        assert!(rc.contains("missing/logs/"), "{rc}");
    }

    fn config(root: &Path, bazel: &Path) -> Config {
        Config {
            root: root.to_path_buf(),
            bazel: bazel.to_path_buf(),
            heap: DEFAULT_HEAP.to_owned(),
            offline: false,
            shim: Some(runfile("DX_TEST_NESTED_BAZEL_SHIM")),
            cache: Vec::new(),
            record: None,
        }
    }

    fn recorder(dir: &Path) -> (PathBuf, PathBuf) {
        let bazel = dir.join("real-bazel");
        std::fs::copy(probe(), &bazel).expect("copy probe");
        make_executable(&bazel).expect("chmod");
        (bazel.clone(), bazel.with_extension("log"))
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

        fn tmpdir(&self) -> PathBuf {
            self.dir.path().join("tmp")
        }

        fn config(&self, vars: &[(&'static str, &str)]) -> Config {
            let mut all = vec![
                (TMPDIR_ENV, self.tmpdir().into_os_string()),
                (PINNED_ENV, self.dir.path().join("bazel").into_os_string()),
            ];
            all.extend(
                vars.iter()
                    .map(|(key, value)| (*key, OsString::from(value))),
            );
            Config::from_parts(&lookup(all), &resolve).expect("config")
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
    fn rules_dx_module_keeps_attrs_and_alias() {
        let text = rules_dx_module(", version = \"1\"", "bazel_dep(name = \"lib\")\n");
        assert!(text.contains("repo_name = \"rules_dx\""), "{text}");
        assert!(text.contains("version = \"1\""), "{text}");
        assert!(text.ends_with("bazel_dep(name = \"lib\")\n"), "{text}");
    }

    const LOCK: &str = r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {"https://bcr.bazel.build/a": "abc"},
  "selectedYankedVersions": {"x@1": "why"},
  "moduleExtensions": {"//:ext.bzl%ext": {"general": {}}},
  "facts": {"@@mod+//:ext.bzl%ext": {"sums": "abc"}},
  "factsVersions": {}
}"#;

    #[test]
    fn seed_lock_json_keeps_facts_and_empties_resolution() {
        let seeded: serde_json::Value =
            serde_json::from_str(&seed_lock_json(LOCK).expect("seed")).expect("json");
        let original: serde_json::Value = serde_json::from_str(LOCK).expect("json");
        assert_eq!(seeded["lockFileVersion"], 28);
        assert_eq!(seeded["facts"], original["facts"]);
        assert_eq!(seeded["factsVersions"], original["factsVersions"]);
        for key in [
            "registryFileHashes",
            "selectedYankedVersions",
            "moduleExtensions",
        ] {
            assert_eq!(seeded[key], serde_json::json!({}), "{key}");
        }
    }

    #[test]
    fn seed_lock_json_rejects_unusable_locks() {
        for (lock, needle) in [
            ("not json", "MODULE.bazel.lock"),
            ("[]", "not a JSON object"),
            (r#"{"facts": {"k": 1}}"#, "lockFileVersion"),
            (r#"{"lockFileVersion": 28}"#, "facts"),
            (r#"{"lockFileVersion": 28, "facts": {}}"#, "facts"),
        ] {
            let error = seed_lock_json(lock).expect_err(lock);
            assert!(error.contains(needle), "{lock}: {error}");
        }
    }

    #[test]
    fn seed_rules_dx_lock_writes_the_consumer_lock() {
        let rules = scratch("dx-seed-rules-");
        let root = scratch("dx-seed-root-");
        std::fs::write(rules.path().join("MODULE.bazel.lock"), LOCK).expect("write lock");
        seed_rules_dx_lock(root.path(), rules.path());
        let text = std::fs::read_to_string(root.path().join("MODULE.bazel.lock")).expect("read");
        assert_eq!(text, seed_lock_json(LOCK).expect("seed"));
    }

    #[test]
    fn rules_dx_config_names_every_flag() {
        let dir = std::env::temp_dir().join(format!("dx-nested-config-{}", std::process::id()));
        write_rules_dx_config(&dir);
        let build = std::fs::read_to_string(dir.join("config/BUILD.bazel")).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        for (name, _) in RULES_DX_FLAGS {
            assert!(build.contains(&format!("name = \"{name}\"")), "{build}");
        }
    }

    #[test]
    fn blocked_download_names_the_url_and_the_cache_sets() {
        let run = finished(1, &format!("Loading\n{BLOCKED_LINE}\nFAILED\n"));
        let message = blocked_download(true, &run).expect("message");
        assert!(message.starts_with(BLOCKED_LINE.trim()), "{message}");
        assert!(message.contains("SHA256SUMS"), "{message}");
        assert!(message.contains("nested_repository_cache"), "{message}");
        assert!(
            message.contains("bazel run //tools/testing:nested_cache_update"),
            "{message}"
        );
    }

    #[test]
    fn blocked_download_is_silent_online_on_success_and_without_the_marker() {
        assert_eq!(blocked_download(false, &finished(1, BLOCKED_LINE)), None);
        assert_eq!(blocked_download(true, &finished(0, BLOCKED_LINE)), None);
        assert_eq!(
            blocked_download(true, &finished(1, "ERROR: no such target\n")),
            None
        );
    }

    #[test]
    fn capture_panics_on_an_offline_blocked_download() {
        let command = probe_command(&["emit", BLOCKED_LINE, "1"]);
        let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capture(command, "probe", true)
        })) {
            Ok(run) => panic!("no panic: {}", run.combined()),
            Err(panic) => panic.downcast_ref::<String>().cloned().unwrap_or_default(),
        };
        assert!(message.starts_with("nested bazel: "), "{message}");
        assert!(message.contains("nested_repository_cache"), "{message}");
        let online = probe_command(&["emit", BLOCKED_LINE, "1"]);
        let run = capture(online, "probe", false);
        assert!(!run.status.success());
    }

    #[test]
    fn the_config_puts_the_root_under_the_test_tmpdir_and_fails_without_one() {
        let fixture = Fixture::new();
        assert_eq!(fixture.config(&[]).root, fixture.tmpdir().join("nested"));
        let bazel = fixture.dir.path().join("bazel").into_os_string();
        let error = Config::from_parts(&lookup(vec![(PINNED_ENV, bazel)]), &resolve)
            .expect_err("no tmpdir");
        assert_eq!(
            error,
            "TEST_TMPDIR is not set; a nested Bazel runs only inside a test"
        );
    }

    #[test]
    fn the_config_reads_each_cache_manifest_and_resolves_its_files() {
        let fixture = Fixture::new();
        let sha256 = "a".repeat(64);
        let manifest = fixture.dir.path().join("set.manifest");
        std::fs::write(&manifest, format!("{sha256} repo/file/file\n")).expect("manifest");
        let config = fixture.config(&[(cache::MANIFESTS_ENV, &manifest.to_string_lossy())]);
        assert_eq!(config.cache, vec![(sha256, PathBuf::from("repo/file/file"))]);
        assert!(fixture.config(&[]).cache.is_empty());
    }

    #[test]
    fn the_config_is_offline_unless_the_network_is_asked_for() {
        let fixture = Fixture::new();
        assert!(fixture.config(&[]).offline);
        assert!(
            fixture
                .config(&[("DX_NESTED_BAZEL_NETWORK", "")])
                .offline
        );
        assert!(
            !fixture
                .config(&[("DX_NESTED_BAZEL_NETWORK", "1")])
                .offline
        );
    }

    #[test]
    fn the_config_takes_the_heap_override() {
        let fixture = Fixture::new();
        assert_eq!(fixture.config(&[]).heap, "1536m");
        assert_eq!(
            fixture
                .config(&[("DX_NESTED_BAZEL_HEAP", "")])
                .heap,
            "1536m"
        );
        assert_eq!(
            fixture
                .config(&[("DX_NESTED_BAZEL_HEAP", "4g")])
                .heap,
            "4g"
        );
    }

    #[test]
    fn a_process_is_owned_only_when_its_command_line_names_the_user_root() {
        let root = Path::new("/nested/owned/root");
        let mut child = probe_command(&["sleep", "30", &root.to_string_lossy()])
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

    fn env_of(command: &Command, key: &str) -> Option<OsString> {
        command
            .get_envs()
            .find(|(name, _)| *name == OsStr::new(key))
            .and_then(|(_, value)| value.map(OsStr::to_owned))
    }

    #[test]
    fn the_command_home_is_the_slot_home_unless_the_caller_names_one() {
        let slot = Path::new("slot");
        let mut command = Command::new("dx");
        apply_env_at(slot, &mut command, &[], &[]);
        let slot_home = Some(slot.join(HOME_DIR).into_os_string());
        assert_eq!(env_of(&command, "HOME"), slot_home);
        assert_eq!(env_of(&command, "USERPROFILE"), slot_home);

        let mut command = Command::new("dx");
        apply_env_at(slot, &mut command, &[("HOME", "caller-home")], &[]);
        assert_eq!(env_of(&command, "HOME"), Some(OsString::from("caller-home")));
        assert_eq!(env_of(&command, "USERPROFILE"), slot_home);
    }

    #[test]
    fn the_wrapper_pins_the_startup_options_and_the_rc_names_the_repository_cache() {
        let config = config(Path::new("/b"), Path::new("/usr/bin/bazel"));
        assert_eq!(
            shim_args(&wrapper_argv(
                &config,
                Path::new("/b/0/u"),
                Path::new("/b/0/bazelrc")
            ))
            .expect("args"),
            "/usr/bin/bazel\n--output_user_root=/b/0/u\n--max_idle_secs=5\n--host_jvm_args=-Xmx1536m\n--bazelrc=/b/0/bazelrc\n"
        );
        let cache = Path::new("/b/repository-cache");
        assert_eq!(
            bazelrc(&config, Path::new("/b/downloader.cfg"), cache),
            "common --local_resources=cpu=2\ncommon --disk_cache=\ncommon --remote_cache=\ncommon --repository_cache=/b/repository-cache\ncommon --repo_env=BAZEL_HTTP_RULES_URLS_AS_DEFAULT_CANONICAL_ID=0\nbuild --spawn_strategy=local\n"
        );
        let offline = Config {
            offline: true,
            ..config
        };
        assert_eq!(
            bazelrc(&offline, Path::new("/b/downloader.cfg"), cache),
            "common --local_resources=cpu=2\ncommon --downloader_config=/b/downloader.cfg\ncommon --lockfile_mode=error\ncommon --disk_cache=\ncommon --remote_cache=\ncommon --repository_cache=/b/repository-cache\ncommon --repo_env=BAZEL_HTTP_RULES_URLS_AS_DEFAULT_CANONICAL_ID=0\nbuild --spawn_strategy=local\n"
        );
    }

    #[test]
    fn pinned_bazel_rejects_a_missing_binary() {
        let dir = scratch("nested-pinned-missing-");
        let missing = dir.path().join("missing/bazel");
        assert_eq!(
            pinned_bazel(missing.clone()).expect_err("missing pinned"),
            format!("DX_NESTED_BAZEL_PINNED {} is not a file", missing.display())
        );
    }

    #[test]
    fn the_config_fails_without_a_pinned_binary_whatever_is_on_path() {
        let dir = scratch("nested-unpinned-");
        for name in ["bazel", "bazelisk"] {
            std::fs::write(dir.path().join(name), "").expect("write");
        }
        let host = dir.path().join("bazel").into_os_string();
        for pinned in [None, Some(OsString::new())] {
            let mut all = vec![
                (TMPDIR_ENV, dir.path().join("tmp").into_os_string()),
                ("DX_NESTED_BAZEL", host.clone()),
                ("PATH", dir.path().as_os_str().to_owned()),
            ];
            all.extend(pinned.map(|value| (PINNED_ENV, value)));
            assert_eq!(
                Config::from_parts(&lookup(all), &resolve).expect_err("unpinned"),
                "DX_NESTED_BAZEL_PINNED is not set; declare the test with nested_bazel_test"
            );
        }
    }

    #[test]
    fn config_reports_a_pinned_key_missing_from_the_runfiles() {
        let fixture = Fixture::new();
        let all = vec![
            (TMPDIR_ENV, fixture.tmpdir().into_os_string()),
            (PINNED_ENV, OsString::from("nested_bazel_x/file/bazel")),
        ];
        let error = Config::from_parts(
            &lookup(all),
            &|name, key| Err(format!("{name} {key} is not in the runfiles")),
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
            (TMPDIR_ENV, fixture.tmpdir().into_os_string()),
            (PINNED_ENV, pinned.clone().into_os_string()),
        ];
        let config = Config::from_parts(&lookup(all), &resolve).expect("config");
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
    fn the_server_lives_in_its_root_and_commands_go_through_the_wrapper() {
        let dir = scratch("nested-root-");
        let (bazel, log) = recorder(dir.path());
        let base = dir.path().join("base");
        let download = dir.path().join("download");
        std::fs::write(&download, "bytes").expect("download");
        let sha256 = "b".repeat(64);
        let declared = Config {
            cache: vec![(sha256.clone(), download)],
            ..config(&base, &bazel)
        };
        let mut first = Server::acquire(declared).expect("server");
        assert_eq!(first.home(), base);
        assert_eq!(first.user_root(), base.join("u"));
        assert!(base.join("home").is_dir());
        assert_eq!(
            std::fs::read_to_string(cache::entry(&first.repository_cache(), &sha256))
                .expect("cache entry"),
            "bytes"
        );
        assert_eq!(
            std::fs::read_to_string(first.home().join("downloader.cfg")).expect("downloader"),
            DOWNLOADER
        );
        assert_eq!(
            std::fs::read_to_string(first.home().join("bazelrc")).expect("bazelrc"),
            bazelrc(
                &first.config,
                &first.home().join("downloader.cfg"),
                &first.repository_cache()
            )
        );

        let root = first.enter("ws").expect("enter");
        std::fs::write(root.join("MODULE.bazel"), "").expect("write");
        let run = capture(
            first.bazel_command(&root, &["info", "release"]),
            "bazel",
            true);
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
    fn the_shim_runs_the_pinned_bazel_with_the_startup_options() {
        let shim = runfile("DX_TEST_NESTED_BAZEL_SHIM");
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
            true);
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
        let env = probe();
        let env = env.as_path();
        let command = server
            .command(env, &[("PATH", "/custom"), ("KEEP", "1")])
            .expect("command");
        let run = capture(command, "env", false);
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
    fn commands_get_no_host_path_and_a_home_in_the_root() {
        let dir = scratch("nested-nopath-");
        let (bazel, _) = recorder(dir.path());
        let server = Server::acquire(config(&dir.path().join("base"), &bazel)).expect("server");
        let command = server
            .command(&probe(), &[])
            .expect("command");
        let run = capture(command, "env", false);
        let lines: Vec<&str> = run.stdout.lines().collect();
        let want_path = format!("PATH={}:/dx_no_path", server.home().join("bin").display());
        let want_home = format!("HOME={}", server.home().join("home").display());
        assert!(lines.contains(&want_path.as_str()), "{}", run.stdout);
        assert!(lines.contains(&want_home.as_str()), "{}", run.stdout);
    }

    #[test]
    fn front_directories_go_ahead_of_the_wrapper_on_path() {
        let dir = scratch("nested-front-");
        let (bazel, _) = recorder(dir.path());
        let server = Server::acquire(config(&dir.path().join("base"), &bazel)).expect("slot");
        let front = dir.path().join("shim");
        let command = server
            .command_with_front(&probe(), &[("PATH", "/custom")], &[&front])
            .expect("command");
        let run = capture(command, "env", false);
        let wrapper_dir = server.home().join("bin");
        let want = format!("PATH={}:{}:/custom", front.display(), wrapper_dir.display());
        assert!(
            run.stdout.lines().any(|line| line == want),
            "{}",
            run.stdout
        );
    }

    fn stand_in(workspace: &Path) -> Child {
        probe_command(&[
            "sleep",
            "60",
            &format!("--workspace_directory={}", workspace.display()),
            "--output_base=/home/u/.cache/bazel/_bazel_u/abc",
        ])
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
        let workspaces = dir.path().join("w");
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
        let workspace = dir.path().join("w/consumer");
        std::fs::create_dir_all(&workspace).expect("mkdir");
        let mut stray = stand_in(&workspace);
        let pid = stray.id();
        wait_until("the stand-in", || {
            servers_in_workspaces(&dir.path().join("w")) == vec![pid]
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
        let home = dir.path().to_path_buf();
        let workspaces = home.join("w");
        std::fs::create_dir_all(&workspaces).expect("mkdir");
        let mut owner = probe_command(&["sleep", "60"]).spawn().expect("owner");
        let mut stray = stand_in(&workspaces.join("consumer"));
        let pid = stray.id();
        wait_until("the stand-in", || {
            servers_in_workspaces(&workspaces) == vec![pid]
        });
        let mut reaper = spawn_reaper(&runfile("DX_TEST_NESTED_BAZEL_SHIM"), owner.id(), &home).expect("reaper");
        std::thread::sleep(Duration::from_millis(1500));
        assert!(alive(pid));
        owner.kill().expect("kill owner");
        let _ = owner.wait();
        wait_until("the reaper to stop the stand-in", || !alive(pid));
        let _ = stray.wait();
        let _ = reaper.wait();
    }

    #[cfg(unix)]
    #[test]
    fn ps_snapshot_finds_servers_by_workspace() {
        let dir = scratch("nested-ps-");
        let workspaces = dir.path().join("w");
        let child = stand_in(&workspaces.join("consumer"));
        let pid = child.id();
        wait_until("stand-in in ps", || {
            servers_in_workspaces_from(&Processes::from_ps(), &workspaces) == vec![pid]
        });
        stop(child, &workspaces);
    }

    #[cfg(unix)]
    #[test]
    fn ps_snapshot_treats_zombies_as_gone() {
        let mut child = probe_command(&["exit", "0"]).spawn().expect("spawn true");
        let pid = child.id();
        wait_until("zombie in ps", || {
            Processes::from_ps()
                .ps
                .as_ref()
                .is_some_and(|t| t.iter().any(|(p, s, _)| *p == pid && *s == 'Z'))
        });
        assert!(!Processes::from_ps().alive(pid));
        let _ = child.wait();
    }
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    #[test]
    fn consumer_module_appends_extra_after_override() {
        let text = consumer_module(Path::new("/src/rules_dx"), "\nuse_repo(x)\n");
        assert_eq!(
            text,
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src/rules_dx\",\n)\n\nuse_repo(x)\n"
        );
    }

    #[test]
    fn files_skip_bazel_links_and_lockfile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_testing::write_text(&root.join("BUILD.bazel"), "");
        dx_testing::write_text(&root.join("MODULE.bazel.lock"), "lock");
        dx_testing::write_text(&root.join("bazel-out/x"), "out");
        dx_testing::write_text(&root.join("bazel-bin"), "bin");
        dx_testing::write_text(&root.join("sub/MODULE.bazel.lock"), "kept");
        let names: Vec<PathBuf> = files_under(root).into_iter().map(|(p, _)| p).collect();
        assert_eq!(
            names,
            vec![
                PathBuf::from("BUILD.bazel"),
                PathBuf::from("sub/MODULE.bazel.lock")
            ]
        );
    }

    #[test]
    fn files_are_sorted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_testing::write_text(&root.join("c"), "3");
        dx_testing::write_text(&root.join("a/b"), "2");
        dx_testing::write_text(&root.join("a/a"), "1");
        assert_eq!(
            files_under(root),
            vec![
                (PathBuf::from("a/a"), b"1".to_vec()),
                (PathBuf::from("a/b"), b"2".to_vec()),
                (PathBuf::from("c"), b"3".to_vec()),
            ]
        );
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
            root: PathBuf::from("/b"),
            bazel: PathBuf::from("/r/bazel"),
            heap: DEFAULT_HEAP.to_owned(),
            offline: true,
            shim: None,
            cache: Vec::new(),
            record: None,
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
        assert!(
            !std::fs::metadata(&dst)
                .expect("meta")
                .permissions()
                .readonly()
        );
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
    fn ps_lines_parse() {
        let text = "    1 Ss   /sbin/init\n 4242 Z+   java  --workspace_directory=/w a  b\nnoise\n\n  x S y\n";
        assert_eq!(
            parse_ps_lines(text),
            vec![
                (1, 'S', "/sbin/init".to_owned()),
                (4242, 'Z', "java  --workspace_directory=/w a  b".to_owned()),
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

    #[cfg(unix)]
    #[test]
    fn alive_treats_an_unreaped_child_as_dead() {
        let mut child = probe_command(&["exit", "0"]).spawn().expect("spawn");
        let pid = child.id();
        let start = std::time::Instant::now();
        while alive(pid) && start.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!alive(pid), "exited but unreaped child counted as alive");
        child.wait().expect("reap");
    }

    /// Env names that a test target sets itself, so the root bazelrc need not forward them.
    const TARGET_SET_ENV: &[&str] = &[
        "DX_NESTED_BAZEL_NETWORK",
        "DX_NESTED_BAZEL_PINNED",
        "DX_NESTED_BAZEL_SHIM",
    ];

    /// The `DX_NESTED_BAZEL*` names read by `from_parts`.
    fn env_names_from_parts() -> Vec<String> {
        let code = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let mut names = Vec::new();
        for (at, _) in code.match_indices("\"DX_NESTED_BAZEL") {
            let rest = &code[at + 1..];
            if let Some(end) = rest.find('"') {
                let name = &rest[..end];
                if name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') {
                    names.push(name.to_owned());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn root_bazelrc_forwards_each_env_name_from_parts_reads() {
        let key = std::env::var("DX_TEST_ROOT_BAZELRC").expect("DX_TEST_ROOT_BAZELRC");
        let path = resolve_runfile("DX_TEST_ROOT_BAZELRC", &key).expect("root bazelrc");
        let rc = dx_testing::read_text(&path);
        let forwarded: Vec<&str> = rc
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                line.strip_prefix("test --test_env=")
                    .or_else(|| line.strip_prefix("common --test_env="))
            })
            .collect();
        let names = env_names_from_parts();
        assert!(
            names.iter().any(|name| name == "DX_NESTED_BAZEL_HEAP"),
            "no env names found in from_parts: {names:?}"
        );
        for name in names
            .iter()
            .filter(|name| !TARGET_SET_ENV.contains(&name.as_str()))
        {
            assert!(
                forwarded.contains(&name.as_str()),
                "{name} is read by from_parts but the root .bazelrc has no `test --test_env={name}`"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_reaper_stops_servers_after_the_owner_is_killed() {
        let scratch = tempfile::tempdir().expect("tempdir");
        let home = scratch.path().to_path_buf();
        let workspaces = home.join(WORKSPACES);
        std::fs::create_dir_all(&workspaces).expect("mkdir");
        let flag = format!(
            "--workspace_directory={}",
            workspaces.join("consumer").display()
        );
        let mut owner = probe_command(&["sleep", "60"]).spawn().expect("owner");
        let mut stray = probe_command(&["sleep", "60", &flag])
            .spawn()
            .expect("stray");
        let pid = stray.id();
        let wait_until = |what: &str, done: &dyn Fn() -> bool| {
            let started = Instant::now();
            while !done() {
                assert!(
                    started.elapsed() < Duration::from_secs(30),
                    "timed out waiting for {what}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        wait_until("the stand-in", &|| {
            servers_in_workspaces(&workspaces).contains(&pid)
        });
        let mut reaper = spawn_reaper(&runfile("DX_TEST_NESTED_BAZEL_SHIM"), owner.id(), &home)
            .expect("reaper");
        let log = || std::fs::read_to_string(home.join(REAPER_LOG)).unwrap_or_default();
        std::thread::sleep(Duration::from_millis(1500));
        let early = reaper.try_wait();
        assert!(
            matches!(early, Ok(None)),
            "the reaper exited before the owner did: {early:?}\nreaper.log:\n{}",
            log()
        );
        assert!(alive(pid));
        owner.kill().expect("kill owner");
        let _ = owner.wait();
        let started = Instant::now();
        while alive(pid) {
            if started.elapsed() >= Duration::from_secs(30) {
                panic!(
                    "timed out waiting for the reaper to stop the stand-in\nreaper: {:?}\nreaper.log:\n{}\nservers_in_workspaces: {:?}",
                    reaper.try_wait(),
                    log(),
                    servers_in_workspaces(&workspaces)
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = stray.wait();
        let _ = reaper.wait();
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_server_under_the_workspaces_is_found_from_the_native_process_table() {
        let scratch = tempfile::tempdir().expect("tempdir");
        let workspace = scratch.path().join("ws");
        let flag = format!("--workspace_directory={}", workspace.display());
        let mut child = probe_command(&["sleep", "30", &flag])
            .spawn()
            .expect("spawn");
        let pid = child.id();
        let started = Instant::now();
        while !servers_in_workspaces(scratch.path()).contains(&pid)
            && started.elapsed() < Duration::from_secs(5)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        let found = servers_in_workspaces(scratch.path()).contains(&pid);
        let elsewhere = servers_in_workspaces(&scratch.path().join("other")).contains(&pid);
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert!(found);
        assert!(!elsewhere);
    }
}

#[cfg(test)]
mod suspend_tests {
    use super::*;
    use std::time::Duration;

    fn heartbeat(file: &Path) -> std::process::Child {
        let mut command = probe_command(&["heartbeat", &file.to_string_lossy()]);
        command.spawn().expect("spawn heartbeat")
    }

    fn size(file: &Path) -> u64 {
        std::fs::metadata(file).map_or(0, |meta| meta.len())
    }

    fn wait_for_growth(file: &Path, from: u64) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if size(file) > from {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn suspend_stops_a_child_and_resume_restarts_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("beat");
        let mut child = heartbeat(&file);
        assert!(wait_for_growth(&file, 0));
        let mut frozen = suspend(child.id()).expect("suspend");
        std::thread::sleep(Duration::from_millis(200));
        let held = size(&file);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(size(&file), held, "the child kept running while suspended");
        assert!(frozen.resume());
        assert!(wait_for_growth(&file, held), "the child did not restart");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn dropping_the_guard_resumes_the_child() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("beat");
        let mut child = heartbeat(&file);
        assert!(wait_for_growth(&file, 0));
        let frozen = suspend(child.id()).expect("suspend");
        std::thread::sleep(Duration::from_millis(200));
        let held = size(&file);
        drop(frozen);
        assert!(wait_for_growth(&file, held), "the child did not restart");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn suspending_a_dead_pid_fails() {
        let mut child = heartbeat(&std::env::temp_dir().join("nested-suspend-dead"));
        let pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        assert!(suspend(pid).is_err());
    }
}
