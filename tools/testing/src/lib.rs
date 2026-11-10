#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};

pub mod cli;
pub mod fatbin;
pub mod http;

pub use dx_binfmt::{machine as image_machine, machines as image_machines};

pub use serde_json;

#[derive(Debug)]
pub struct Run {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn combined(&self) -> String {
        let mut out = self.stdout.clone();
        out.push_str(&self.stderr);
        out
    }
}

/// Runs one tool on the runfiles built beside it, never on the runfiles of this process.
///
/// Returns a `Command` for `bin` with `envs` set and the runfiles variables pointing at `bin`'s own
/// runfiles, so callers can add arguments and choose how to collect output.
pub fn command_for(bin: &Path, envs: &[(&str, &str)]) -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(bin);
    command.envs(envs.iter().copied());
    match dx_path::Resolver::for_binary(bin) {
        Ok(own) => {
            let source = own.source().to_path_buf();
            if source.is_dir() {
                command.env_remove("RUNFILES_MANIFEST_FILE");
                command.env("RUNFILES_DIR", source);
            } else {
                command.env("RUNFILES_MANIFEST_FILE", source);
                command.env_remove("RUNFILES_DIR");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(command)
}

const DEFAULT_CHILD_TIMEOUT_SECS: u64 = 120;

fn child_timeout() -> std::time::Duration {
    let secs = std::env::var("DX_TESTING_CHILD_TIMEOUT_SECS")
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(DEFAULT_CHILD_TIMEOUT_SECS);
    std::time::Duration::from_secs(secs)
}

/// Runs `bin` like `run`, with a default deadline of 120s (`DX_TESTING_CHILD_TIMEOUT_SECS`).
pub fn run(bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::io::Result<Run> {
    run_within(bin, args, envs, child_timeout())
}

type Captured = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;

fn drain<R: std::io::Read + Send + 'static>(
    mut pipe: R,
    into: Captured,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = pipe.read(&mut chunk) {
            if count == 0 {
                break;
            }
            into.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend_from_slice(&chunk[..count]);
        }
    })
}

fn captured_text(buffer: &Captured) -> String {
    let bytes = buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    String::from_utf8_lossy(&bytes).into_owned()
}

fn wait_for(handle: std::thread::JoinHandle<()>, limit: std::time::Duration) {
    let end = std::time::Instant::now() + limit;
    while !handle.is_finished() && std::time::Instant::now() < end {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    if handle.is_finished() {
        let _ = handle.join();
    }
}

/// Runs `bin` and kills it when it outlives `limit`, returning `TimedOut` with the output so far.
pub fn run_within(
    bin: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
    limit: std::time::Duration,
) -> std::io::Result<Run> {
    run_command(command_for(bin, envs)?.args(args), limit)
}

/// Like `run_within` for a prepared command; the caller sets any platform flags.
pub fn run_command(
    command: &mut std::process::Command,
    limit: std::time::Duration,
) -> std::io::Result<Run> {
    use std::process::Stdio;
    let program = PathBuf::from(command.get_program());
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let out = Captured::default();
    let err = Captured::default();
    let readers = [
        child.stdout.take().map(|pipe| drain(pipe, out.clone())),
        child.stderr.take().map(|pipe| drain(pipe, err.clone())),
    ];
    let deadline = std::time::Instant::now() + limit;
    let mut status = None;
    while status.is_none() {
        status = child.try_wait()?;
        if status.is_none() {
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let timed_out = status.is_none();
    if timed_out {
        let _ = child.kill();
        status = Some(child.wait()?);
    }
    // A descendant may still hold the pipes open after the child is gone.
    let grace = if timed_out {
        std::time::Duration::from_secs(2)
    } else {
        std::time::Duration::from_secs(30)
    };
    for reader in readers.into_iter().flatten() {
        wait_for(reader, grace);
    }
    let run = Run {
        status: status.ok_or_else(|| std::io::Error::other("child has no exit status"))?,
        stdout: captured_text(&out),
        stderr: captured_text(&err),
    };
    if timed_out {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!(
                "{} outlived {limit:?} and was killed\nstdout:\n{}\nstderr:\n{}",
                program.display(),
                run.stdout,
                run.stderr
            ),
        ));
    }
    Ok(run)
}

static SCRATCH_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Being removed by #2841; use `dx_test_scratch::scratch`.
pub fn mkscratch(prefix: &str) -> std::io::Result<PathBuf> {
    let base = std::env::var("TEST_TMPDIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let seq = SCRATCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = base.join(format!("{prefix}-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

pub fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    text.push('\n');
    std::fs::write(path, text.as_bytes())
        .map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// Writes `text` to `path`, creating its parents and panicking with the path on failure.
pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

/// Reads `path` as UTF-8 text, panicking with the path on failure.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn read_lines(path: &Path) -> std::io::Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.split('\n').map(str::to_owned).collect())
}

const POWERSHELL_SUFFIXES: [&str; 1] = [".ps1"];

fn suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        &POWERSHELL_SUFFIXES
    } else {
        &[]
    }
}

fn suffixed(key: &str) -> Vec<String> {
    if Path::new(key).extension().is_some() {
        return Vec::new();
    }
    dx_path::host::EXECUTABLE_SUFFIXES
        .iter()
        .copied()
        .chain(suffixes().iter().copied())
        .map(|suffix| format!("{key}{suffix}"))
        .collect()
}

fn workspace() -> String {
    std::env::var("TEST_WORKSPACE")
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "_main".to_owned())
}

/// Returns the runfiles key that one `$(rootpath)` or `$(rlocationpath)` spells.
pub fn rlocation_key(rel: &str) -> String {
    if let Some(external) = rel.strip_prefix("../") {
        return external.to_owned();
    }
    let workspace = workspace();
    if rel
        .strip_prefix(workspace.as_str())
        .is_some_and(|rest| rest.starts_with('/'))
    {
        return rel.to_owned();
    }
    format!("{workspace}/{rel}")
}

fn runfiles_source() -> String {
    for name in ["RUNFILES_MANIFEST_FILE", "RUNFILES_DIR", "TEST_SRCDIR"] {
        if let Some(value) = std::env::var_os(name).filter(|value| !value.is_empty()) {
            return format!("{name}={}", Path::new(&value).display());
        }
    }
    "the runfiles beside argv[0]".to_owned()
}

fn runfiles_tree() -> Option<PathBuf> {
    ["RUNFILES_DIR", "TEST_SRCDIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .find(|dir| dir.is_dir())
}

fn existing(runfiles: &runfiles::Runfiles, key: &str) -> Option<PathBuf> {
    runfiles
        .rlocation_from(key, "")
        .filter(|path| path.exists())
}

/// Resolves one runfiles key through the Bazel runfiles of this process.
pub fn rlocation(key: &str) -> std::io::Result<PathBuf> {
    let opened = runfiles::Runfiles::create();
    if let Some(found) = opened
        .as_ref()
        .ok()
        .and_then(|runfiles| existing(runfiles, key))
    {
        return Ok(found);
    }
    if let Some(tree) = runfiles_tree() {
        let directory = runfiles::Runfiles::builder().directory(&tree).build();
        if let Some(found) = directory.ok().and_then(|runfiles| existing(&runfiles, key)) {
            return Ok(found);
        }
    }
    let detail = match opened {
        Ok(_) => format!("{key} is not in {}", runfiles_source()),
        Err(error) => format!("cannot open {} for {key}: {error}", runfiles_source()),
    };
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, detail))
}

/// Resolves one `$(rootpath)` or `$(rlocationpath)` test input, panicking with its key when absent.
pub fn resolve_runfiles(rel: &str) -> PathBuf {
    find_runfile(rel).unwrap_or_else(|error| panic!("runfile {rel}: {error}"))
}

/// Resolves the runfile named by environment variable `var`, panicking when it is unset.
pub fn env_file(var: &str) -> PathBuf {
    resolve_runfiles(&env_text(var))
}

/// Reads environment variable `var`, panicking when it is unset.
pub fn env_text(var: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| panic!("{var} is set"))
}

/// Resolves every whitespace separated runfile named by environment variable `var`.
pub fn env_files(var: &str) -> Vec<PathBuf> {
    env_text(var)
        .split_whitespace()
        .map(resolve_runfiles)
        .collect()
}

/// Renders a path as an owned string.
pub fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Creates and returns the directory `name` under `TEST_TMPDIR`.
pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env_text("TEST_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    dir
}

/// Joins the runfiles named by environment variables `vars` into a classpath string.
pub fn classpath(vars: &[&str]) -> String {
    let joined = std::env::join_paths(vars.iter().map(|var| env_file(var)))
        .unwrap_or_else(|e| panic!("classpath entries join: {e}"));
    joined.to_string_lossy().into_owned()
}

/// Reads one test input resolved through `resolve_runfiles`, panicking when it cannot be read.
pub fn read_runfile(rel: &str) -> Vec<u8> {
    let resolved = resolve_runfiles(rel);
    std::fs::read(&resolved).unwrap_or_else(|e| panic!("read {}: {e}", resolved.display()))
}

/// Reports whether `needle` occurs in `haystack`.
pub fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn find_runfile(rel: &str) -> std::io::Result<PathBuf> {
    let direct = Path::new(rel);
    if direct.is_absolute() {
        return Ok(direct.to_path_buf());
    }
    let key = rlocation_key(rel);
    let error = match rlocation(&key) {
        Ok(found) => return Ok(found),
        Err(error) => error,
    };
    suffixed(&key)
        .iter()
        .find_map(|candidate| rlocation(candidate).ok())
        .ok_or(error)
}

/// Names the rules_dx checkout behind the `//:MODULE.bazel` runfile of the calling test.
pub fn rules_dx_source() -> PathBuf {
    let module = find_runfile("MODULE.bazel").unwrap_or_else(|error| {
        panic!(
            "runfile MODULE.bazel: {error}; add //:module_srcs or an area module_srcs to the test's data"
        )
    });
    let real = std::fs::canonicalize(&module).unwrap_or_else(|error| panic!("{module:?}: {error}"));
    real.parent()
        .unwrap_or_else(|| panic!("no checkout above {real:?}"))
        .to_path_buf()
}

/// Writes the `@dx_test_tags_sources` snapshot named by `DX_SOURCES_JSON`, `DX_FILES_JSON` and, when set, `DX_RUST_JSON` and `DX_PROSE_JSON` into a scratch tree and names its root.
pub fn source_snapshot() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let root = mkscratch("source-snapshot").expect("snapshot scratch dir");
        for variable in [
            "DX_SOURCES_JSON",
            "DX_FILES_JSON",
            "DX_PROSE_JSON",
            "DX_RUST_JSON",
        ] {
            let optional = matches!(variable, "DX_PROSE_JSON" | "DX_RUST_JSON");
            if optional && std::env::var_os(variable).is_none() {
                continue;
            }
