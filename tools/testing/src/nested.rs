use std::cell::{Ref, RefCell};
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::{Rc, Weak};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};
use std::time::{Duration, Instant};

use crate::Run;

pub const MAX_IDLE_SECS: u32 = 5;
pub const DEFAULT_HEAP: &str = "1536m";
pub const DEFAULT_SLOTS: usize = 3;
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
    pub disk_cache: Option<PathBuf>,
    pub repository_cache: Option<PathBuf>,
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
        let base = match env_path("DX_NESTED_BAZEL_ROOT") {
            Some(base) => base,
            None => default_base()?,
        };
        let slots = std::env::var("DX_NESTED_BAZEL_SLOTS")
            .ok()
            .and_then(|slots| slots.parse::<usize>().ok())
            .filter(|slots| *slots > 0)
            .unwrap_or(DEFAULT_SLOTS);
        let heap = std::env::var("DX_NESTED_BAZEL_HEAP")
            .ok()
            .filter(|heap| !heap.is_empty())
            .unwrap_or_else(|| DEFAULT_HEAP.to_owned());
        let path = std::env::var_os("PATH").unwrap_or_default();
        let bazel = find_bazel(&path, &base)?;
        let disk_cache = env_path("DX_NESTED_BAZEL_DISK_CACHE")
            .or_else(|| rc_path(&rc, "disk_cache", home.as_deref()));
        let repository_cache = env_path("DX_NESTED_BAZEL_REPOSITORY_CACHE")
            .or_else(|| rc_path(&rc, "repository_cache", home.as_deref()))
            .or_else(|| home.as_deref().and_then(default_repository_cache));
        Ok(Config {
            base,
            slots,
            bazel,
            heap,
            disk_cache,
            repository_cache,
        })
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(unix)]
fn default_base() -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let probe = crate::mkscratch("nested-bazel-uid")
        .map_err(|error| format!("cannot create a scratch directory: {error}"))?;
    let uid = std::fs::metadata(&probe)
        .map_err(|error| format!("cannot stat {}: {error}", probe.display()))?
        .uid();
    let _ = std::fs::remove_dir(&probe);
    Ok(PathBuf::from(format!("/tmp/dxnb-{uid}")))
}

#[cfg(not(unix))]
fn default_base() -> Result<PathBuf, String> {
    Ok(std::env::temp_dir().join("dxnb"))
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

/// Finds the real `bazel` or `bazelisk` on `path`, skipping the harness wrappers.
pub fn find_bazel(path: &std::ffi::OsStr, base: &Path) -> Result<PathBuf, String> {
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
    Err("no bazel or bazelisk on PATH".to_owned())
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
pub fn bazelrc(config: &Config) -> String {
    let mut text = format!("common --local_resources=cpu={CPUS}\n");
    if let Some(cache) = &config.disk_cache {
        text.push_str(&format!("common --disk_cache={}\n", cache.display()));
    }
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

fn owned_by(pid: u32, user_root: &Path) -> bool {
    match std::fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(cmdline) => String::from_utf8_lossy(&cmdline).contains(&*user_root.to_string_lossy()),
        Err(_) => !Path::new("/proc/self").exists(),
    }
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

/// Stops every server under `user_root` and returns the ones that survive.
pub fn kill_servers(user_root: &Path) -> Vec<u32> {
    let pids = servers(user_root);
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
                let lock = home.join("lock");
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
        let server = Server {
            config,
            home,
            workspace: None,
            _slot: slot,
        };
        kill_servers(&server.user_root());
        let rc = server.home.join("bazelrc");
        write(&rc, &bazelrc(&server.config))?;
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
        self.home.join("u")
    }

    pub fn wrapper(&self) -> PathBuf {
        self.home.join("bin").join("bazel")
    }

    /// Empties the workspace `name` and makes it current, stopping the server of any other one.
    pub fn enter(&mut self, name: &str) -> Result<PathBuf, String> {
        let root = self.home.join("w").join(name);
        if self.workspace.as_ref() != Some(&root) {
            self.shutdown();
            self.workspace = Some(root.clone());
        }
        reset(&root)?;
        Ok(root)
    }

    /// Lists the live servers this slot owns.
    pub fn servers(&self) -> Vec<u32> {
        servers(&self.user_root())
    }

    /// Stops the server and returns any that survive.
    pub fn shutdown(&mut self) -> Vec<u32> {
        if let Some(root) = self.workspace.as_ref().filter(|root| root.is_dir()) {
            if !self.servers().is_empty() {
                let _ = self.bazel_command(root, &["shutdown"]).output();
            }
        }
        kill_servers(&self.user_root())
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
        bazel_command_at(&self.home, root, args)
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

fn bazel_command_at(home: &Path, root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(home.join("bin").join("bazel"));
    command.current_dir(root).args(STARTUP).args(args);
    apply_env_at(home, &mut command, &[]);
    command
}

impl Drop for Server {
    fn drop(&mut self) {
        let left = self.shutdown();
        if !left.is_empty() {
            eprintln!("nested Bazel servers survived shutdown: {left:?}");
        }
    }
}

fn create_private(base: &Path) -> Result<(), String> {
    std::fs::create_dir_all(base)
        .map_err(|error| format!("cannot create {}: {error}", base.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(base, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn capture(mut command: Command, what: &str) -> Run {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("run {what}: {error}"));
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
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

static HOME: Mutex<Option<PathBuf>> = Mutex::new(None);

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
            *HOME.lock().unwrap_or_else(PoisonError::into_inner) = Some(server.home().to_owned());
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
    let home = HOME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| panic!("nested bazel: no case has started"));
    capture(bazel_command_at(&home, root, args), "nested bazel")
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
        capture(command, &bin.display().to_string())
    }

    /// Runs `bazel` in the workspace with the startup options dx uses.
    pub fn bazel(&self, args: &[&str]) -> Run {
        bazel_in(&self.root, args)
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn scratch(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("scratch tempdir")
    }

    fn config(base: &Path, bazel: &Path) -> Config {
        Config {
            base: base.to_path_buf(),
            slots: 2,
            bazel: bazel.to_path_buf(),
            heap: DEFAULT_HEAP.to_owned(),
            disk_cache: Some(PathBuf::from("/caches/disk")),
            repository_cache: None,
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
    fn the_wrapper_pins_the_startup_options_and_the_rc_names_the_caches() {
        let config = config(Path::new("/b"), Path::new("/usr/bin/bazel"));
        let script = wrapper_script(&config, Path::new("/b/0/u"), Path::new("/b/0/bazelrc"));
        assert_eq!(
            script,
            "#!/bin/sh\nexec '/usr/bin/bazel' '--output_user_root=/b/0/u' '--max_idle_secs=5' '--host_jvm_args=-Xmx1536m' '--bazelrc=/b/0/bazelrc' \"$@\"\n"
        );
        assert_eq!(
            bazelrc(&config),
            "common --local_resources=cpu=2\ncommon --disk_cache=/caches/disk\n"
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
            find_bazel(&path, &dir.path().join("base")).expect("found"),
            real.join("bazel")
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

        let root = first.enter("ws").expect("enter");
        std::fs::write(root.join("MODULE.bazel"), "").expect("write");
        let run = capture(first.bazel_command(&root, &["info", "release"]), "bazel");
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
        let run = capture(command, "env");
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
}
