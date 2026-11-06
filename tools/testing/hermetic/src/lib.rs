#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use dx_nested::compose::{compose_module, family_modules};
use serde_json::{json, Value};

/// Returns the lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(data))
}

pub const SCHEMA: &str = "dx.hermetic.v1";

pub const AMBIENT_TOOLS: &[&str] = &[
    "cc",
    "gcc",
    "c++",
    "g++",
    "cpp",
    "clang",
    "clang++",
    "ld",
    "ld.lld",
    "ld.gold",
    "as",
    "ar",
    "ranlib",
    "nm",
    "strip",
    "objcopy",
    "objdump",
    "python",
    "python3",
    "perl",
    "ruby",
    "gem",
    "node",
    "npm",
    "npx",
    "yarn",
    "pnpm",
    "java",
    "javac",
    "go",
    "rustc",
    "cargo",
    "rustup",
    "dotnet",
    "mono",
    "swift",
    "swiftc",
    "kotlinc",
    "scalac",
    "pip",
    "pip3",
    "make",
    "cmake",
    "ninja",
    "curl",
    "wget",
    "git",
    "tar",
    "unzip",
    "xz",
    "bzip2",
    "zstd",
    "7z",
    "7za",
    "nvcc",
    "vale",
    "rst2html",
    "rst2html.py",
    "rst2html5",
    "asciidoctor",
    "typst",
    "typstyle",
    "latexindent",
    "latexindent.pl",
    "chktex",
    "pandoc",
];

pub const TOOL_DIRS: &[&str] = &["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];

pub const IP_CANDIDATES: &[&str] = &["/usr/sbin/ip", "/sbin/ip", "/usr/bin/ip", "/bin/ip"];

pub const SDK_DIRS: &[&str] = &[
    "/usr/local",
    "/opt",
    "/usr/include",
    "/usr/lib/gcc",
    "/usr/libexec/gcc",
];

pub const SDK_DIR_PREFIXES: &[(&str, &str)] = &[
    ("/usr/lib", "llvm-"),
    ("/usr/lib", "clang"),
    ("/usr/lib", "cuda"),
    ("/usr/lib", "nvidia-cuda-toolkit"),
];

/// Lists the host SDK directories, including versioned LLVM trees.
pub fn sdk_candidates() -> Vec<PathBuf> {
    if cfg!(windows) {
        return windows_host_dirs();
    }
    let mut dirs: Vec<PathBuf> = SDK_DIRS.iter().map(PathBuf::from).collect();
    for (parent, prefix) in SDK_DIR_PREFIXES {
        let Ok(entries) = std::fs::read_dir(parent) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .map(|entry| entry.path())
            .collect();
        found.sort();
        dirs.extend(found);
    }
    dirs
}

/// The host locations `Isolation::plan_with` probes, so tests can supply fakes.
#[derive(Debug, Clone)]
pub struct HostPaths {
    pub ip_candidates: Vec<PathBuf>,
    pub sdk_candidates: Vec<PathBuf>,
    pub tool_dirs: Vec<PathBuf>,
}

impl HostPaths {
    /// Reads the live host: the real iproute2, SDK and tool locations.
    pub fn detect() -> Self {
        Self {
            ip_candidates: IP_CANDIDATES.iter().map(PathBuf::from).collect(),
            sdk_candidates: sdk_candidates(),
            tool_dirs: if cfg!(windows) {
                windows_host_dirs()
            } else {
                TOOL_DIRS.iter().map(PathBuf::from).collect()
            },
        }
    }
}

pub const HOME_CONFIG: &[&str] = &[
    ".bazelrc",
    ".cargo",
    ".config",
    ".gitconfig",
    ".gradle",
    ".local",
    ".m2",
    ".netrc",
    ".npmrc",
    ".rustup",
];

pub const ACTION_PATH: &str = "/usr/bin:/bin";

#[cfg(unix)]
pub const SHIM: &str = "#!/bin/sh\necho \"dx-hermetic: undeclared ambient tool $0\" >&2\nexit 97\n";

#[cfg(windows)]
pub const SHIM: &str =
    "@echo off\r\necho dx-hermetic: undeclared ambient tool %0 1>&2\r\nexit /b 97\r\n";

/// The file name of the ambient-tool shim.
pub const SHIM_NAME: &str = if cfg!(windows) {
    "ambient-tool.cmd"
} else {
    "ambient-tool"
};

/// The PATH of every action: system directories only, plus the shim directory on Windows.
pub fn action_path(system_root: Option<&str>, shim_dir: Option<&Path>) -> String {
    if !cfg!(windows) {
        return ACTION_PATH.to_owned();
    }
    let root = system_root.unwrap_or(r"C:\Windows");
    let mut dirs = vec![format!(r"{root}\System32"), root.to_owned()];
    dirs.extend(shim_dir.map(|dir| dir.to_string_lossy().into_owned()));
    dirs.join(";")
}

/// The bash a Windows Bazel runs genrules with: `BAZEL_SH`, then the first candidate that
/// exists. Anything under the system root is skipped, since System32's bash.exe is the WSL stub.
pub fn windows_bash(
    bazel_sh: Option<&Path>,
    system_root: &str,
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    let root = PathBuf::from(system_root.to_lowercase());
    bazel_sh
        .into_iter()
        .chain(candidates.iter().map(PathBuf::as_path))
        .filter(|path| !PathBuf::from(path.to_string_lossy().to_lowercase()).starts_with(&root))
        .find(|path| path.is_file())
        .map(Path::to_path_buf)
}

/// The Windows tool directories under Program Files and the local profile: Python, Node, JDK and LLVM.
pub fn windows_tool_dirs(
    program_files: Option<&Path>,
    local_app_data: Option<&Path>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut versioned_under = |parent: PathBuf, prefix: &str, sub: &str| {
        let Ok(entries) = std::fs::read_dir(&parent) else {
            return;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .map(|entry| entry.path().join(sub))
            .collect();
        found.sort();
        dirs.extend(found);
    };
    if let Some(root) = program_files {
        versioned_under(root.to_path_buf(), "Python", "");
        versioned_under(root.join("Java"), "jdk", "bin");
        versioned_under(root.join("Eclipse Adoptium"), "jdk", "bin");
        versioned_under(root.to_path_buf(), "nodejs", "");
        versioned_under(root.to_path_buf(), "LLVM", "bin");
    }
    if let Some(local) = local_app_data {
        versioned_under(local.join("Programs").join("Python"), "Python", "");
    }
    dirs.into_iter()
        .filter_map(|dir| canonical(&dir))
        .filter(|dir| dir.is_dir())
        .collect()
}

/// Resolves symlinks and drops the `\\?\` prefix Windows adds, so paths compare equal.
fn canonical(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok().map(strip_verbatim)
}

/// Drops a `\\?\` prefix from a drive path and leaves UNC and POSIX paths alone.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

/// Reads a directory variable, falling back to the default location when Bazel's test environment lacks it.
fn windows_dir(name: &str, fallback: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).or_else(fallback)
}

/// The directories the Windows host keeps compilers and interpreters in.
fn windows_host_dirs() -> Vec<PathBuf> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    let program_files = windows_dir("ProgramFiles", || {
        Some(PathBuf::from(format!(r"{drive}\Program Files")))
    });
    let local = windows_dir("LOCALAPPDATA", || {
        std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(r"AppData\Local"))
    });
    windows_tool_dirs(program_files.as_deref(), local.as_deref())
}

pub const AMBIENT_MARKER: &str = "dx-hermetic: undeclared ambient tool";

pub const NETWORK_MARKER: &str = "Network is unreachable";

pub const HOME_MARKER: &str = "dx-hermetic: home config absent";

/// Marker strings the probes expect, by host OS: (ambient tool, network, home config).
pub fn markers(os: &str) -> (&'static str, &'static str, &'static str) {
    if os == "macos" {
        (MACOS_DENIED_MARKER, MACOS_DENIED_MARKER, HOME_MARKER)
    } else if os == "windows" {
        (AMBIENT_MARKER, WINDOWS_DENIED_MARKER, HOME_MARKER)
    } else {
        (AMBIENT_MARKER, NETWORK_MARKER, HOME_MARKER)
    }
}

/// Host variables a Windows action still needs: the system root and the Visual Studio locations Bazel reads.
pub const WINDOWS_PASSTHROUGH: &[&str] = &[
    "SystemRoot",
    "windir",
    "ComSpec",
    "PATHEXT",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramData",
    "BAZEL_VC",
    "BAZEL_VC_FULL_VERSION",
    "BAZEL_LLVM",
];

/// How a probe starts on Windows, where the environment and directory are set on the process instead of by `env` and `sh`.
pub struct WindowsRun {
    pub env: Vec<(String, String)>,
    pub shutdown: Vec<String>,
}

pub const APPCONTAINER_ENV: &str = "DX_HERMETIC_APPCONTAINER";

pub const MACOS_DENIED_MARKER: &str = "Operation not permitted";

/// WSAEACCES, which an AppContainer without network capabilities gets from connect.
pub const WINDOWS_DENIED_MARKER: &str = "os error 10013";

pub const MISS_MARKER: &str = "Unknown host";

/// The sandbox denies the registry connection on macOS before DNS can fail.
pub fn miss_marker(os: &str) -> &'static str {
    if os == "macos" {
        MACOS_DENIED_MARKER
    } else {
        MISS_MARKER
    }
}

pub const HOME_RC: &str = "build --action_env=DX_HOME_CONFIG=present\n";

pub const LEAKS_PACKAGE: &str = "dx_hermetic_leaks";

pub const REQUIRED_PROBES: &[&str] = &[
    "fetch",
    "hermetic_test",
    "artifacts",
    "ambient_tool",
    "network",
    "home_config",
    "home_config_control",
    "warm_cache",
    "cache_miss",
];

/// Probes that run dx itself inside the isolation, once the manifest names coverage patterns.
pub const DX_PROBES: &[&str] = &["dx_generate_check", "dx_coverage"];

const BUILD_FLAGS: &[&str] = &[
    "--noshow_progress",
    "--color=no",
    "--curses=no",
    "--incompatible_strict_action_env",
    "--sandbox_default_allow_network=false",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub label: String,
    pub languages: Vec<String>,
    pub module: String,
    pub repo_name: String,
    pub workspace_dir: String,
    pub patterns: Vec<String>,
    pub failures: Vec<(String, String)>,
    pub coverage: Vec<String>,
    pub covered: Vec<String>,
    pub generate: bool,
    pub files: Vec<(PathBuf, String)>,
    pub module_fragment: Option<PathBuf>,
    pub families: Vec<String>,
    pub compiler: Option<(String, String)>,
    pub platform: String,
    pub profile: String,
    pub lint_findings: Vec<(String, u64)>,
    pub dx: PathBuf,
    pub module_file: PathBuf,
    pub bazelversion: PathBuf,
}

fn strings(value: &Value, key: &str) -> Result<Vec<String>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("manifest field {key} must be a list"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("manifest field {key} holds a non-string"))
        })
        .collect()
}

fn string(value: &Value, key: &str) -> Result<String, String> {
    match value[key].as_str() {
        Some(text) if !text.is_empty() => Ok(text.to_owned()),
        _ => Err(format!("manifest field {key} must be a non-empty string")),
    }
}

/// Rejects a consumer path that is absolute or climbs out of the workspace.
pub fn checked_dest(dest: &str) -> Result<String, String> {
    let bad = dest.is_empty()
        || dest.starts_with('/')
        || dest.contains('\\')
        || dest
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(format!(
            "consumer path {dest:?} must stay inside the workspace"
        ));
    }
    Ok(dest.to_owned())
}

/// Names the probe that builds one expected-failure target.
pub fn failure_probe_name(label: &str) -> String {
    let body: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("fails_{body}")
}

fn parse_failures(value: &Value) -> Result<Vec<(String, String)>, String> {
    let mut failures = Vec::new();
    for (label, marker) in value["failures"]
        .as_object()
        .ok_or_else(|| "manifest field failures must be a map".to_owned())?
    {
        if !label.starts_with("//") {
            return Err(format!("failure target {label} must be a //label"));
        }
        let marker = marker
            .as_str()
            .filter(|marker| !marker.is_empty())
            .ok_or_else(|| format!("failure target {label} needs a non-empty marker"))?;
        failures.push((label.clone(), marker.to_owned()));
    }
    failures.sort();
    let mut names: Vec<String> = failures
        .iter()
        .map(|(label, _)| failure_probe_name(label))
        .collect();
    names.sort();
    if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!(
            "two failure targets share the probe name {}",
            pair[0]
        ));
    }
    Ok(failures)
}

fn parse_coverage(
    value: &Value,
    files: &[(PathBuf, String)],
) -> Result<(Vec<String>, Vec<String>), String> {
    let coverage = strings(value, "coverage")?;
    if let Some(bad) = coverage.iter().find(|pattern| !pattern.starts_with("//")) {
        return Err(format!(
            "coverage pattern {bad} must be a positive //pattern"
        ));
    }
    let covered = strings(value, "covered")?;
    if coverage.is_empty() != covered.is_empty() {
        return Err("manifest fields coverage and covered must be set together".to_owned());
    }
    for path in &covered {
        checked_dest(path)?;
        if !files.iter().any(|(_, dest)| dest == path) {
            return Err(format!("covered path {path} is not a consumer file"));
        }
    }
    Ok((coverage, covered))
}

fn parse_lint_findings(value: &Value) -> Result<Vec<(String, u64)>, String> {
    let Some(map) = value["lint_findings"].as_object() else {
        return Ok(Vec::new());
    };
    let mut findings = Vec::new();
    for (key, count) in map {
        if !key.contains(':') {
            return Err(format!("lint_findings key {key} must be tool:rule"));
        }
        let count = count
            .as_str()
            .and_then(|text| text.parse::<u64>().ok())
            .ok_or_else(|| format!("lint_findings count for {key} must be a whole number"))?;
        findings.push((key.clone(), count));
    }
    findings.sort();
    Ok(findings)
}

fn parse_profile(value: &Value) -> Result<String, String> {
    let profile = value["profile"].as_str().unwrap_or("");
    if !["", "standard", "strict"].contains(&profile) {
        return Err(format!(
            "manifest profile {profile} must be standard or strict"
        ));
    }
    Ok(profile.to_owned())
}

/// Parses the manifest a qualification target writes, resolving paths against `base`.
pub fn parse_manifest(text: &str, base: &Path) -> Result<Manifest, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("manifest is not JSON: {err}"))?;
    let mut files = Vec::new();
    for entry in value["files"]
        .as_array()
        .ok_or_else(|| "manifest field files must be a list".to_owned())?
    {
        let src = entry["src"]
            .as_str()
            .ok_or_else(|| "manifest file entry needs src".to_owned())?;
        let dest = entry["dest"]
            .as_str()
            .ok_or_else(|| "manifest file entry needs dest".to_owned())?;
        files.push((base.join(src), checked_dest(dest)?));
    }
    let patterns = strings(&value, "patterns")?;
    if patterns.is_empty() {
        return Err("manifest names no consumer target patterns".to_owned());
    }
    if let Some(negative) = patterns.iter().find(|pattern| pattern.starts_with('-')) {
        return Err(format!(
            "manifest pattern {negative} is negative; list only the targets to qualify"
        ));
    }
    let languages = strings(&value, "languages")?;
    if languages.is_empty() {
        return Err("manifest names no languages".to_owned());
    }
    let failures = parse_failures(&value)?;
    let (coverage, covered) = parse_coverage(&value, &files)?;
    let fragment = value["module_fragment"].as_str().unwrap_or("");
    let families = if value["families"].is_null() {
        Vec::new()
    } else {
        strings(&value, "families")?
    };
    let compiler = match value["compiler"]["name"]
        .as_str()
        .filter(|name| !name.is_empty())
    {
        Some(name) => Some((name.to_owned(), string(&value["compiler"], "producer")?)),
        None => None,
    };
    let repo_name = string(&value, "repo_name")?;
    let module_fragment = if fragment.is_empty() {
        None
    } else {
        Some(base.join(fragment))
    };
    if module_fragment.is_none() {
        return Err("manifest needs a module_fragment".to_owned());
    }
    Ok(Manifest {
        label: string(&value, "label")?,
        languages,
        module: string(&value, "module")?,
        repo_name,
        workspace_dir: checked_dest(&string(&value, "workspace_dir")?)?,
        patterns,
        failures,
        coverage,
        covered,
        generate: value["generate"].as_bool().unwrap_or(false),
        files,
        module_fragment,
        families,
        compiler,
        platform: value["platform"].as_str().unwrap_or("").to_owned(),
        profile: parse_profile(&value)?,
        lint_findings: parse_lint_findings(&value)?,
        dx: base.join(string(&value, "dx")?),
        module_file: base.join(string(&value, "module_file")?),
        bazelversion: base.join(string(&value, "bazelversion")?),
    })
}

/// Returns the leak package that proves each isolation boundary.
pub fn leaks_build(tool: &str) -> String {
    format!(
        "genrule(\n    name = \"ambient_tool\",\n    outs = [\"ambient_tool.txt\"],\n    cmd = \"{tool} --version > $@\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"home_config\",\n    outs = [\"home_config.txt\"],\n    cmd = \"test \\\"$${{DX_HOME_CONFIG:-absent}}\\\" = present || {{ echo dx-hermetic: home config $${{DX_HOME_CONFIG:-absent}} >&2; exit 1; }}; echo present > $@\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"network\",\n    outs = [\"network.txt\"],\n    cmd = \"exec 3<>/dev/tcp/1.1.1.1/443 && echo connected > $@\",\n    tags = [\n        \"manual\",\n        \"requires-network\",\n    ],\n)\n"
    )
}

/// Returns the leak package with PowerShell genrules, for a Windows host.
pub fn leaks_build_windows(tool: &str) -> String {
    let ambient = format!("{tool} --version | Out-File -FilePath '$@'; exit $$LASTEXITCODE");
    let home = "if ($$env:DX_HOME_CONFIG -eq 'present') { Set-Content -Path '$@' -Value present } else { [Console]::Error.WriteLine('dx-hermetic: home config absent'); exit 1 }";
    let network = "try { $$null = New-Object System.Net.Sockets.TcpClient('1.1.1.1', 443); Set-Content -Path '$@' -Value connected } catch { $$e = $$_.Exception; if ($$e.InnerException) { $$e = $$e.InnerException }; [Console]::Error.WriteLine('connect failed: os error ' + $$e.NativeErrorCode); exit 1 }";
    format!(
        "genrule(\n    name = \"ambient_tool\",\n    outs = [\"ambient_tool.txt\"],\n    cmd_ps = \"{ambient}\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"home_config\",\n    outs = [\"home_config.txt\"],\n    cmd_ps = \"{home}\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"network\",\n    outs = [\"network.txt\"],\n    cmd_ps = \"{network}\",\n    tags = [\n        \"manual\",\n        \"requires-network\",\n    ],\n)\n"
    )
}

/// Returns the leak package for the host this qualification runs on.
pub fn leaks_build_host(tool: &str) -> String {
    if cfg!(windows) {
        leaks_build_windows(tool)
    } else {
        leaks_build(tool)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    Pass,
    /// Exit 0, or exit 1 when the linter reported findings.
    Findings,
    Fail(String),
}

/// Reports whether one probe ended the way the qualification requires.
pub fn verdict(expect: &Expect, exit: i32, log: &str) -> Result<(), String> {
    match expect {
        Expect::Pass if exit == 0 => Ok(()),
        Expect::Pass => Err(format!("exited {exit}, want 0")),
        Expect::Findings if exit == 0 || exit == 1 => Ok(()),
        Expect::Findings => Err(format!("exited {exit}, want 0 or 1")),
        Expect::Fail(_) if exit == 0 => Err("succeeded, want a failure".to_owned()),
        Expect::Fail(marker) if log.contains(marker.as_str()) => Ok(()),
        Expect::Fail(marker) => Err(format!("exited {exit} without {marker:?}")),
    }
}

/// Names a core WebAssembly module wasm32, or None for any other file.
pub fn wasm_machine(head: &[u8]) -> Option<String> {
    (head.len() >= 8 && head[..8] == *b"\0asm\x01\0\0\0").then(|| "wasm32".to_owned())
}

/// Quotes one word for a POSIX shell.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Reads the home directory of one user from passwd text.
pub fn passwd_home(passwd: &str, uid: u32) -> Option<PathBuf> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() >= 7 && fields[2] == uid.to_string()).then(|| PathBuf::from(fields[5]))
    })
}

/// Reads the real user or group id from /proc/self/status text.
pub fn status_id(status: &str, key: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|id| id.parse().ok())
}

fn versioned(entry: &str, name: &str) -> bool {
    entry
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// Finds every ambient tool and its versioned variants under the given directories, resolved and deduplicated.
pub fn ambient_tools(dirs: &[PathBuf], names: &[&str]) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for name in names {
        for dir in dirs {
            let mut entries = vec![(*name).to_owned()];
            if let Ok(listing) = std::fs::read_dir(dir) {
                let mut extra: Vec<String> = listing
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|entry| versioned(entry, name))
                    .collect();
                extra.sort();
                entries.extend(extra);
            }
            for entry in entries {
                let file = format!("{entry}{}", std::env::consts::EXE_SUFFIX);
                let Some(real) = canonical(&dir.join(&file)) else {
                    continue;
                };
                if real.is_file() && !found.iter().any(|(_, seen)| *seen == real) {
                    found.push((entry, real));
                }
            }
        }
    }
    found
}

fn covers(outer: &Path, inner: &Path) -> bool {
    inner.starts_with(outer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Net {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, Default)]
pub struct Isolation {
    pub shim: PathBuf,
    pub tools: Vec<PathBuf>,
    pub sdk_dirs: Vec<PathBuf>,
    pub home_dirs: Vec<PathBuf>,
    pub home_files: Vec<PathBuf>,
    pub ip: PathBuf,
    pub uid: u32,
    pub gid: u32,
    /// The Windows launcher, from `DX_HERMETIC_APPCONTAINER`.
    pub launcher: PathBuf,
    /// The paths the Windows run may use: everything the run itself needs.
    pub grants: Vec<PathBuf>,
    /// Paths the Windows user backend must not read, even where the account could.
    pub denies: Vec<PathBuf>,
}

impl Isolation {
    /// Plans the masks for this host, keeping every path the run itself needs.
    /// Probes the live host; see `plan_with` for the injectable form.
    pub fn plan(shim: &Path, keep: &[PathBuf], home: Option<&Path>) -> Result<Self, String> {
        Self::plan_with(&HostPaths::detect(), shim, keep, home)
    }

    /// Plans the masks against the given host locations.
    pub fn plan_with(
        host: &HostPaths,
        shim: &Path,
        keep: &[PathBuf],
        home: Option<&Path>,
    ) -> Result<Self, String> {
        let (uid, gid) = real_ids()?;
        let ip = if cfg!(any(target_os = "macos", windows)) {
            PathBuf::new()
        } else {
            host.ip_candidates
                .iter()
                .find(|path| path.is_file())
                .cloned()
                .ok_or("namespace isolation needs ip from iproute2 to raise loopback")?
        };
        let kept = |path: &Path| keep.iter().any(|needed| covers(path, needed));
        let sdk_dirs: Vec<PathBuf> = host
            .sdk_candidates
            .iter()
            .filter(|dir| dir.is_dir() && !kept(dir))
            .cloned()
            .collect();
        let tools = ambient_tools(&host.tool_dirs, AMBIENT_TOOLS)
            .into_iter()
            .map(|(_, path)| path)
            .filter(|path| !sdk_dirs.iter().any(|dir| covers(dir, path)) && !kept(path))
            .collect();
        let mut home_dirs = Vec::new();
        let mut home_files = Vec::new();
        if let Some(home) = home {
            for entry in HOME_CONFIG {
                let path = home.join(entry);
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if kept(&path) {
                    continue;
                }
                if meta.is_dir() {
                    home_dirs.push(path);
                } else {
                    home_files.push(path);
                }
            }
        }
        Ok(Self {
            shim: shim.to_path_buf(),
            tools,
            sdk_dirs,
            home_dirs,
            home_files,
            ip,
            uid,
            gid,
            launcher: std::env::var_os(APPCONTAINER_ENV)
                .map(PathBuf::from)
                .unwrap_or_default(),
            grants: keep.to_vec(),
            denies: Vec::new(),
        })
    }

    /// Returns the namespace setup script; "$@" is the command run as the real user.
    pub fn script(&self, net: Net) -> String {
        let mut lines = vec!["set -eu".to_owned()];
        if net == Net::Denied {
            lines.push(format!(
                "{} link set lo up",
                shell_quote(&self.ip.to_string_lossy())
            ));
        }
        for dir in self.sdk_dirs.iter().chain(&self.home_dirs) {
            lines.push(format!(
                "mount -t tmpfs dx-hermetic {}",
                shell_quote(&dir.to_string_lossy())
            ));
        }
        for file in &self.home_files {
            lines.push(format!(
                "mount --bind /dev/null {}",
                shell_quote(&file.to_string_lossy())
            ));
        }
        for tool in &self.tools {
            lines.push(format!(
                "mount --bind {} {}",
                shell_quote(&self.shim.to_string_lossy()),
                shell_quote(&tool.to_string_lossy())
            ));
        }
        lines.push(format!(
            "exec unshare --user --map-user={} --map-group={} -- \"$@\"",
            self.uid, self.gid
        ));
        lines.join("\n") + "\n"
    }

    /// Wraps one command in fresh user, mount and, when denied, network namespaces.
    pub fn command(&self, net: Net, inner: &[String]) -> Command {
        if cfg!(target_os = "macos") {
            return self.macos_command(net, inner);
        }
        if cfg!(windows) {
            return self.windows_command(net, inner);
        }
        let mut command = Command::new("/usr/bin/unshare");
        command.env_clear();
        command.env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin");
        command.args(["--user", "--map-root-user", "--mount"]);
        if net == Net::Denied {
            command.arg("--net");
        }
        command.args(["--", "/bin/sh", "-c", &self.script(net), "dx-hermetic"]);
        command.args(inner);
        command
    }
}

/// Quotes a path for a sandbox profile string, resolving symlinks like `/var`.
fn profile_path(path: &Path) -> String {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

impl Isolation {
    /// Wraps one command in a per-run throwaway Windows account made by the launcher.
    pub fn windows_command(&self, net: Net, inner: &[String]) -> Command {
        static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut command = Command::new(&self.launcher);
        command.args([
            "--name",
            &format!("dx-hermetic-{}-{run}", std::process::id()),
            "--backend",
            "user",
        ]);
        if net == Net::Allowed {
            command.arg("--net");
        }
        for grant in &self.grants {
            let text = grant.to_string_lossy();
            command.arg("--grant");
            command.arg(text.strip_prefix(r"\\?\").unwrap_or(&text));
        }
        for deny in &self.denies {
            let text = deny.to_string_lossy();
            command.arg("--deny");
            command.arg(text.strip_prefix(r"\\?\").unwrap_or(&text));
        }
        command.arg("--");
        command.args(inner);
        command
    }

    /// The macOS sandbox profile: network denied except loopback when asked, planned paths hidden.
    pub fn macos_profile(&self, net: Net) -> String {
        let mut profile = "(version 1)(allow default)".to_owned();
        if net == Net::Denied {
            profile.push_str("(deny network*)(allow network-outbound (remote ip \"localhost:*\"))(allow network-bind network-inbound (local ip \"localhost:*\"))(allow network* (remote unix-socket))(allow network* (local unix-socket))");
        }
        for dir in self.sdk_dirs.iter().chain(&self.home_dirs) {
            profile.push_str(&format!(
                "(deny file-read* (subpath \"{}\"))",
                profile_path(dir)
            ));
        }
        for file in &self.home_files {
            profile.push_str(&format!(
                "(deny file-read* (literal \"{}\"))",
                profile_path(file)
            ));
        }
        for tool in &self.tools {
            profile.push_str(&format!(
                "(deny process-exec (literal \"{}\"))",
                profile_path(tool)
            ));
        }
        profile
    }

    /// Wraps one command in a macOS `sandbox-exec` profile.
    pub fn macos_command(&self, net: Net, inner: &[String]) -> Command {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command.env_clear();
        command.env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin");
        command.args(["-p", &self.macos_profile(net), "--"]);
        command.args(inner);
        command
    }
}

/// The real user and group id: /proc/self/status on Linux, `id` elsewhere.
fn real_ids() -> Result<(u32, u32), String> {
    if cfg!(windows) {
        return Ok((0, 0));
    }
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let uid = status_id(&status, "Uid:").ok_or("no Uid in /proc/self/status")?;
        let gid = status_id(&status, "Gid:").ok_or("no Gid in /proc/self/status")?;
        return Ok((uid, gid));
    }
    let id = |flag: &str| -> Result<u32, String> {
        let out = Command::new("/usr/bin/id")
            .arg(flag)
            .output()
            .map_err(|err| format!("cannot run id {flag}: {err}"))?;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .map_err(|err| format!("bad id {flag} output: {err}"))
    };
    Ok((id("-u")?, id("-g")?))
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs one command, capturing its exit code and output.
pub fn capture(mut command: Command) -> Result<Outcome, String> {
    let output = command
        .output()
        .map_err(|err| format!("cannot start {command:?}: {err}"))?;
    Ok(Outcome {
        exit: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub struct Probe<'a> {
    pub name: String,
    pub net: Net,
    pub home: &'a Path,
    pub output_root: &'a Path,
    pub repos: &'a Path,
    pub command: &'static str,
    pub targets: Vec<String>,
    pub extra: Vec<String>,
    pub expect: Expect,
}

pub struct Session {
    pub manifest: Manifest,
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub out: PathBuf,
    pub source: PathBuf,
    pub bazelisk: PathBuf,
    /// The binary every probe runs: bazelisk, or on Windows the Bazel it resolved.
    pub bazel: PathBuf,
    pub bazelisk_home: PathBuf,
    pub isolation: Isolation,
    pub probes: Vec<Value>,
    /// The MSYS bash the Windows Bazel runs genrules with.
    pub bash: Option<PathBuf>,
}

impl Session {
    fn platform_flag(&self) -> Option<String> {
        Some(self.manifest.platform.as_str())
            .filter(|platform| !platform.is_empty())
            .map(|platform| format!("--platforms={platform}"))
    }

    fn shell_flag(&self) -> Option<String> {
        self.bash
            .as_ref()
            .filter(|_| cfg!(windows))
            .map(|bash| format!("--shell_executable={}", bash.display()))
    }

    fn action_path(&self) -> String {
        action_path(
            std::env::var("SystemRoot").ok().as_deref(),
            Some(&self.root.join("bin").join("shims")),
        )
    }

    /// The environment a Windows action gets: nothing inherited but the system and toolchain variables.
    pub fn windows_env(&self, home: &Path, path: &str) -> Vec<(String, String)> {
        if !cfg!(windows) {
            return Vec::new();
        }
        let temp = self.root.join("tmp");
        let _ = std::fs::create_dir_all(&temp);
        let mut env = vec![
            ("HOME".to_owned(), home.to_string_lossy().into_owned()),
            (
                "USERPROFILE".to_owned(),
                home.to_string_lossy().into_owned(),
            ),
            ("PATH".to_owned(), path.to_owned()),
            (
                "BAZELISK_HOME".to_owned(),
                self.bazelisk_home.to_string_lossy().into_owned(),
            ),
            ("TEMP".to_owned(), temp.to_string_lossy().into_owned()),
            ("TMP".to_owned(), temp.to_string_lossy().into_owned()),
        ];
        for key in WINDOWS_PASSTHROUGH {
            if let Ok(value) = std::env::var(key) {
                env.push(((*key).to_owned(), value));
            }
        }
        if let Some(bash) = &self.bash {
            env.push(("BAZEL_SH".to_owned(), bash.to_string_lossy().into_owned()));
        }
        env
    }

    /// The targets of the `hermetic_test` probe. With coverage targets it also
    /// builds the generate_check binary, so `dx_generate_check` only runs it.
    pub fn hermetic_test_targets(&self) -> Vec<String> {
        let mut targets = self.manifest.patterns.clone();
        if !self.manifest.coverage.is_empty() {
            targets.push(format!("@{}//dx:generate_check", self.manifest.repo_name));
        }
        targets
    }

    /// The bazelisk command line of one probe, without the process wrapper. Build-like
    /// verbs carry the settings and disk cache dx adds, so dx reuses the probe builds.
    fn bazel_tail(&self, probe: &Probe) -> Vec<String> {
        let mut argv = vec![self.bazel.to_string_lossy().into_owned()];
        argv.extend(startup_flags(probe.output_root));
        argv.extend([
            "--max_idle_secs=60".to_owned(),
            probe.command.to_owned(),
            format!("--repository_cache={}", probe.repos.display()),
        ]);
        argv.extend(BUILD_FLAGS.iter().map(|flag| (*flag).to_owned()));
        if dx_process::takes_shared_settings(probe.command) {
            argv.extend(dx_process::head_settings(&self.manifest.repo_name));
        }
        argv.extend(self.dx_verb_options(probe.home, probe.output_root, probe.command));
        argv.extend(self.platform_flag());
        argv.extend(self.shell_flag());
        argv.extend(probe.extra.iter().cloned());
        argv.push("--".to_owned());
        argv.extend(probe.targets.iter().cloned());
        argv
    }

    /// The cache and gc flags dx adds after `command` for this home and output root.
    fn dx_verb_options(&self, home: &Path, output_root: &Path, command: &str) -> Vec<String> {
        let cache = cache_home(output_root);
        let env_get = |key: &str| match key {
            "HOME" => Some(home.to_string_lossy().into_owned()),
            "XDG_CACHE_HOME" => Some(cache.to_string_lossy().into_owned()),
            _ => None,
        };
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        dx_process::RcPolicy::from_env(&env_get, &self.workspace, &read).verb_options(
            command,
            None,
            &[],
        )
    }

    fn bazel_argv(&self, probe: &Probe) -> Vec<String> {
        let mut argv = vec![
            "/usr/bin/env".to_owned(),
            "-i".to_owned(),
            format!("HOME={}", probe.home.display()),
            format!("PATH={ACTION_PATH}"),
            format!("BAZELISK_HOME={}", self.bazelisk_home.display()),
            format!("XDG_CACHE_HOME={}", cache_home(probe.output_root).display()),
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "cd \"$0\" && \"$@\"; rc=$?; \"$1\" \"$2\" \"$3\" \"$4\" shutdown >/dev/null 2>&1; exit $rc"
                .to_owned(),
            self.workspace.to_string_lossy().into_owned(),
        ];
        argv.extend(self.bazel_tail(probe));
        argv
    }

    // LCOV_EXCL_START - reason: runs only inside the isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
    /// Writes a `bazel` that runs the resolved bazelisk, so dx finds it on its PATH.
    fn bazel_bin(&self, output_root: &Path) -> Result<PathBuf, String> {
        let dir = self.root.join("bin").join("path");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
        if cfg!(windows) {
            let script = format!(
                "@echo off\r\n\"{}\" {} %*\r\n",
                self.bazel.display(),
                startup_flags(output_root).join(" ")
            );
            write(&dir.join("bazel.cmd"), &script)?;
            return Ok(dir);
        }
        let flags: Vec<String> = startup_flags(output_root)
            .iter()
            .map(|flag| shell_quote(flag))
            .collect();
        let script = format!(
            "#!/bin/sh\nexec {} {} \"$@\"\n",
            shell_quote(&self.bazel.to_string_lossy()),
            flags.join(" ")
        );
        let bazel = dir.join("bazel");
        write(&bazel, &script)?;
        executable(&bazel)?;
        Ok(dir)
    }

    /// Runs the dx binary on `targets` with `bazel` on its PATH.
    pub fn run_dx(
        &mut self,
        name: &str,
        net: Net,
        command: &str,
        targets: &[String],
        home: &Path,
        output_root: &Path,
        repos: &Path,
    ) -> Result<Outcome, String> {
        let bin = self.bazel_bin(output_root)?;
        let mut args = vec![command.to_owned()];
        args.extend(targets.iter().cloned());
        if command == "lint" {
            args.extend(["--output".to_owned(), "json".to_owned()]);
        }
        if name == "dx_generate" {
            args.push("--apply".to_owned());
        }
        if name == "dx_generate_check" {
            args.push("--diff".to_owned());
        }
        args.extend([
            "--".to_owned(),
            format!("--repository_cache={}", repos.display()),
        ]);
        if net == Net::Denied {
            args.push("--repository_disable_download".to_owned());
        }
        args.extend(BUILD_FLAGS.iter().map(|flag| (*flag).to_owned()));
        args.extend(self.platform_flag());
        args.extend(self.shell_flag());
        if cfg!(windows) {
            let path = format!("{};{}", bin.display(), self.action_path());
            let mut argv = vec![self.manifest.dx.to_string_lossy().into_owned()];
            argv.extend(args);
            let expect = if command == "lint" {
                Expect::Findings
            } else {
                Expect::Pass
            };
            let mut env = self.windows_env(home, &path);
            env.push(xdg_cache_home(output_root));
            let run = WindowsRun {
                env,
                shutdown: self.windows_shutdown(output_root),
            };
            return self.execute(name, net, home, &argv, argv.clone(), &expect, Some(run));
        }
        let mut argv = vec![
            "/usr/bin/env".to_owned(),
            "-i".to_owned(),
            format!("HOME={}", home.display()),
            format!("PATH={}:{ACTION_PATH}", bin.display()),
            format!("BAZELISK_HOME={}", self.bazelisk_home.display()),
            format!("XDG_CACHE_HOME={}", cache_home(output_root).display()),
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "cd \"$0\" && \"$@\"; rc=$?; bazel --max_idle_secs=60 shutdown >/dev/null 2>&1; exit $rc"
                .to_owned(),
            self.workspace.to_string_lossy().into_owned(),
            self.manifest.dx.to_string_lossy().into_owned(),
        ];
        argv.extend(args);
        let recorded = argv[10..].to_vec();
        // Lint exits 1 when it reports findings, which the manifest then compares.
        let expect = if command == "lint" {
            Expect::Findings
        } else {
            Expect::Pass
        };
        self.execute(name, net, home, &argv, recorded, &expect, None)
    }

    /// Runs one probe, logs it and records its verdict.
    pub fn run(&mut self, probe: Probe) -> Result<Outcome, String> {
        if cfg!(windows) {
            let argv = self.bazel_tail(&probe);
            let mut env = self.windows_env(probe.home, &self.action_path());
            env.push(xdg_cache_home(probe.output_root));
            let run = WindowsRun {
                env,
                shutdown: self.windows_shutdown(probe.output_root),
            };
            return self.execute(
                &probe.name,
                probe.net,
                probe.home,
                &argv,
                argv.clone(),
                &probe.expect,
                Some(run),
            );
        }
        let argv = self.bazel_argv(&probe);
        let recorded = argv[10..].to_vec();
        self.execute(
            &probe.name,
            probe.net,
            probe.home,
            &argv,
            recorded,
            &probe.expect,
            None,
        )
    }

    fn windows_shutdown(&self, output_root: &Path) -> Vec<String> {
        let mut argv = vec![self.bazel.to_string_lossy().into_owned()];
        argv.extend(startup_flags(output_root));
        argv.push("shutdown".to_owned());
        argv
    }

    fn execute(
        &mut self,
        name: &str,
        net: Net,
        home: &Path,
        argv: &[String],
        recorded: Vec<String>,
        expect: &Expect,
        windows: Option<WindowsRun>,
    ) -> Result<Outcome, String> {
        let started = Instant::now();
        let outcome = match windows {
            Some(run) => {
                let launch = |inner: &[String]| {
                    let mut command = self.isolation.command(net, inner);
                    command
                        .env_clear()
                        .envs(run.env.iter().cloned())
                        .current_dir(&self.workspace);
                    capture(command)
                };
                let outcome = launch(argv)?;
                let _ = launch(&run.shutdown);
                outcome
            }
            None => capture(self.isolation.command(net, argv))?,
        };
        let seconds = started.elapsed().as_secs();
        let log = format!("{}{}", outcome.stdout, outcome.stderr);
        let logs = self.out.join("logs");
        std::fs::create_dir_all(&logs).map_err(|err| format!("cannot create logs: {err}"))?;
        let log_path = logs.join(format!("{name}.log"));
        std::fs::write(&log_path, &log)
            .map_err(|err| format!("cannot write {}: {err}", log_path.display()))?;
        let checked = verdict(expect, outcome.exit, &log);
        let (expected, marker) = match expect {
            Expect::Pass | Expect::Findings => ("pass", ""),
            Expect::Fail(marker) => ("fail", marker.as_str()),
        };
        self.probes.push(json!({
            "name": name,
            "network": if net == Net::Denied { "denied" } else { "allowed" },
            "home": home.to_string_lossy(),
            "argv": recorded,
            "expect": expected,
            "marker": marker,
            "exit": outcome.exit,
            "seconds": seconds,
            "log": format!("logs/{name}.log"),
            "verdict": if checked.is_ok() { "ok" } else { "wrong" },
            "detail": checked.as_ref().err().cloned().unwrap_or_default(),
        }));
        checked.map_err(|err| {
            let lines: Vec<&str> = log.lines().collect();
            let log_tail = lines[lines.len().saturating_sub(40)..].join("\n");
            let module =
                std::fs::read_to_string(self.workspace.join("MODULE.bazel")).unwrap_or_default();
            let testlogs = copy_test_logs(
                &self.workspace.join("bazel-testlogs"),
                &logs.join(name).join("testlogs"),
            );
            format!(
                "probe {name} {err}; see {}\nlog tail:\n{}\nconsumer MODULE.bazel:\n{module}{testlogs}",
                log_path.display(),
                log_tail
            )
        })?;
        Ok(outcome)
    }
    // LCOV_EXCL_STOP - reason: end isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
}

/// Copies every consumer `test.log` under `testlogs` to `dest`, keeping the
/// relative layout, and returns the first 40 lines of each for the error text.
pub fn copy_test_logs(testlogs: &Path, dest: &Path) -> String {
    let mut found = Vec::new();
    collect_test_logs(testlogs, &mut found);
    found.sort();
    let mut text = String::new();
    for path in found {
        let Ok(rel) = path.strip_prefix(testlogs) else {
            continue;
        };
        let Ok(content) = std::fs::read(&path) else {
            continue;
        };
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&target, &content);
        let content = String::from_utf8_lossy(&content);
        let head: Vec<&str> = content.lines().take(40).collect();
        text.push_str(&format!(
            "\nconsumer test log {}:\n{}",
            rel.display(),
            head.join("\n")
        ));
    }
    text
}

fn collect_test_logs(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_test_logs(&path, found);
        } else if path.file_name().is_some_and(|name| name == "test.log") {
            found.push(path);
        }
    }
}

/// Builds the cquery expression for the non-manual targets the patterns name.
pub fn built_query(patterns: &[String]) -> String {
    let all = patterns.join(" + ");
    format!("({all}) except attr(tags, \"\\bmanual\\b\", {all})")
}

/// Hashes and identifies every output file cquery listed.
pub fn artifacts(workspace: &Path, listed: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for line in listed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let path = workspace.join(line);
        if path.is_dir() {
            continue;
        }
        let bytes =
            std::fs::read(&path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        out.push(json!({
            "path": line,
            "bytes": bytes.len(),
            "sha256": sha256_hex(&bytes),
            "machine": dx_binfmt::machine(&bytes).or_else(|| wasm_machine(&bytes)),
            "producers": dx_binfmt::producers(&bytes),
        }));
    }
    if out.is_empty() {
        return Err("cquery listed no consumer artifacts".to_owned());
    }
    Ok(out)
}

fn probe_expectation(name: &str, os: &str) -> (&'static str, &'static str) {
    let (ambient, network, home) = markers(os);
    match name {
        "ambient_tool" => ("fail", ambient),
        "network" => ("fail", network),
        "home_config" => ("fail", home),
        "cache_miss" => ("fail", miss_marker(os)),
        _ => ("pass", ""),
    }
}

const NATIVE_LANGUAGES: [&str; 6] = ["appimage", "c", "cpp", "cuda", "rust", "zig"];

fn unique_machines(artifacts: &[Value]) -> Vec<String> {
    let mut machines: Vec<String> = artifacts
        .iter()
        .filter_map(|artifact| artifact["machine"].as_str().map(str::to_owned))
        .collect();
    machines.sort();
    machines.dedup();
    machines
}

/// One source file of a coverage report with its line counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoveredSource {
    pub path: String,
    pub lines_found: u64,
    pub lines_hit: u64,
}

/// Turns coverage patterns into the workspace path prefixes they own.
pub fn owned_prefixes(coverage: &[String]) -> Vec<String> {
    coverage
        .iter()
        .map(|pattern| {
            let body = pattern.trim_start_matches("//");
            let package = match body.strip_suffix("...") {
                Some(rest) => rest.trim_end_matches('/'),
                None => body.split(':').next().unwrap_or_default(),
            };
            if package.is_empty() {
                String::new()
            } else {
                format!("{package}/")
            }
        })
        .collect()
}

/// Reads the per-file line counts of an LCOV report.
pub fn coverage_sources(report: &str) -> Result<Vec<CoveredSource>, String> {
    let files = dx_lcov::parse_lcov(report).map_err(|err| format!("coverage report: {err}"))?;
    Ok(files
        .into_iter()
        .map(|(path, hits)| CoveredSource {
            path,
            lines_found: hits.lines.len() as u64,
            lines_hit: hits.lines.values().filter(|count| **count > 0).count() as u64,
        })
        .collect())
}

/// Reads coverage sources back from recorded evidence.
pub fn sources_from_json(value: &Value) -> Result<Vec<CoveredSource>, String> {
    value
        .as_array()
        .ok_or_else(|| "evidence records no coverage sources".to_owned())?
        .iter()
        .map(|source| {
            Ok(CoveredSource {
                path: source["path"]
                    .as_str()
                    .ok_or_else(|| "coverage source has no path".to_owned())?
                    .to_owned(),
                lines_found: source["lines_found"]
                    .as_u64()
                    .ok_or_else(|| "coverage source has no lines_found".to_owned())?,
                lines_hit: source["lines_hit"]
                    .as_u64()
                    .ok_or_else(|| "coverage source has no lines_hit".to_owned())?,
            })
        })
        .collect()
}

fn source_package_path(path: &str) -> &str {
    let parts: Vec<&str> = path.splitn(4, '/').collect();
    match parts.as_slice() {
        ["bazel-out", _, "bin", rest] => rest,
        _ => path,
    }
}

/// Checks a coverage report holds only owned consumer sources and hits every covered one.
pub fn check_coverage(sources: &[CoveredSource], manifest: &Manifest) -> Result<(), String> {
    if sources.is_empty() {
        return Err("coverage report has no source files".to_owned());
    }
    let prefixes = owned_prefixes(&manifest.coverage);
    for source in sources {
        let path = source.path.as_str();
        if path.starts_with('/')
            || path.ends_with(".jar")
            || path.ends_with(".class")
            || path.split('/').any(|part| part == "external")
        {
            return Err(format!("coverage report names a non-source file {path}"));
        }
        let owned = source_package_path(path);
        if !prefixes
            .iter()
            .any(|prefix| owned.starts_with(prefix.as_str()))
        {
            return Err(format!(
                "coverage report names {path} outside the coverage patterns"
            ));
        }
    }
    for path in &manifest.covered {
        let Some(source) = sources.iter().find(|source| source.path == *path) else {
            return Err(format!("coverage report misses {path}"));
        };
        if source.lines_hit == 0 {
            return Err(format!("coverage report shows no hit lines in {path}"));
        }
    }
    Ok(())
}

/// Counts the `tool:rule` pairs of the diagnostic events in `dx lint --output json` stdout.
pub fn lint_counts(stdout: &str) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for event in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if event["event"] != "diagnostic" {
            continue;
        }
        if let (Some(tool), Some(rule)) = (event["tool"].as_str(), event["rule"].as_str()) {
            *counts.entry(format!("{tool}:{rule}")).or_insert(0) += 1;
        }
    }
    counts
}

fn check_lint(recorded: &Value, want: &[(String, u64)]) -> Result<(), String> {
    let mut got: Vec<(String, u64)> = recorded
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(key, count)| (key.clone(), count.as_u64().unwrap_or(u64::MAX)))
                .collect()
        })
        .unwrap_or_default();
    got.sort();
    if got != want {
        return Err(format!("lint findings {got:?} disagree with {want:?}"));
    }
    Ok(())
}

fn required_probes<'a>(manifest: &'a Manifest, os: &str) -> Vec<(String, &'a str, &'a str, Net)> {
    let mut required: Vec<(String, &str, &str, Net)> = Vec::new();
    if manifest.generate {
        required.push(("dx_generate".to_owned(), "pass", "", Net::Allowed));
    }
    required.extend(REQUIRED_PROBES.iter().map(|name| {
        let (expect, marker) = probe_expectation(name, os);
        let net = if *name == "fetch" {
            Net::Allowed
        } else {
            Net::Denied
        };
        ((*name).to_owned(), expect, marker, net)
    }));
    if !manifest.coverage.is_empty() {
        required.push(("fetch_coverage".to_owned(), "pass", "", Net::Allowed));
        required.push(("coverage".to_owned(), "pass", "", Net::Denied));
        for name in DX_PROBES {
            required.push(((*name).to_owned(), "pass", "", Net::Denied));
        }
    }
    if !manifest.lint_findings.is_empty() {
        required.push(("dx_lint".to_owned(), "pass", "", Net::Denied));
    }
    for (label, marker) in &manifest.failures {
        required.push((failure_probe_name(label), "fail", marker, Net::Denied));
    }
    required
}

/// Checks evidence against the commit and the manifest it must describe.
pub fn verify_evidence(evidence: &Value, commit: &str, manifest: &Manifest) -> Result<(), String> {
    let label = manifest.label.as_str();
    let languages = manifest.languages.as_slice();
    if evidence["schema"] != SCHEMA {
        return Err(format!("evidence schema must be {SCHEMA}"));
    }
    if evidence["qualification"] != label {
        return Err(format!(
            "evidence describes {}, want {label}",
            evidence["qualification"]
        ));
    }
    if evidence["rules_dx"]["commit"] != commit {
        return Err(format!(
            "evidence is stale: it records commit {}, want {commit}",
            evidence["rules_dx"]["commit"]
        ));
    }
    if evidence["rules_dx"]["dirty"] != false {
        return Err(format!(
            "evidence came from a tree with uncommitted changes: {}",
            evidence["rules_dx"]["changes"]
        ));
    }
    let recorded: Vec<&str> = evidence["languages"]
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if languages.is_empty() || recorded != languages {
        return Err(format!(
            "evidence languages {} disagree with the required {languages:?}",
            evidence["languages"]
        ));
    }
    if evidence["result"] != "qualified" {
        return Err(format!("evidence result is {}", evidence["result"]));
    }
    for key in ["os", "arch", "kernel"] {
        if evidence["host"][key]
            .as_str()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(format!("evidence names no host {key}"));
        }
    }
    let probes = evidence["probes"].as_array().cloned().unwrap_or_default();
    for (name, expect, marker, net) in
        required_probes(manifest, evidence["exec"]["os"].as_str().unwrap_or("linux"))
    {
        let Some(probe) = probes.iter().find(|probe| probe["name"] == name.as_str()) else {
            return Err(format!("evidence is missing required probe {name}"));
        };
        if probe["verdict"] != "ok" {
            return Err(format!("required probe {name} did not hold"));
        }
        if probe["expect"] != expect || probe["marker"] != marker {
            return Err(format!(
                "required probe {name} recorded expect {} marker {}, want {expect} {marker:?}",
                probe["expect"], probe["marker"]
            ));
        }
        if net == Net::Denied && probe["network"] != "denied" {
            return Err(format!("required probe {name} ran with network"));
        }
    }
    for (label, _) in &manifest.failures {
        let name = failure_probe_name(label);
        let built = probes
            .iter()
            .filter(|probe| probe["name"] == name.as_str())
            .all(|probe| {
                probe["argv"]
                    .as_array()
                    .is_some_and(|argv| argv.iter().any(|arg| arg == label.as_str()))
            });
        if !built {
            return Err(format!("probe {name} did not build {label}"));
        }
    }
    if !manifest.coverage.is_empty() {
        check_coverage(
            &sources_from_json(&evidence["coverage"]["sources"])?,
            manifest,
        )?;
    }
    if !manifest.lint_findings.is_empty() {
        check_lint(&evidence["lint"], &manifest.lint_findings)?;
    }
    let artifacts = evidence["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if artifacts.is_empty() {
        return Err("evidence names no built artifacts".to_owned());
    }
    for artifact in &artifacts {
        let digest = artifact["sha256"].as_str().unwrap_or_default();
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("artifact {} has no sha256", artifact["path"]));
        }
    }
    if evidence["target"]["machines"] != json!(unique_machines(&artifacts)) {
        return Err("evidence target machines disagree with its artifacts".to_owned());
    }
    if let Some((name, producer)) = &manifest.compiler {
        check_compiler(name, producer, &artifacts)?;
    }
    let native = languages
        .iter()
        .any(|language| NATIVE_LANGUAGES.contains(&language.as_str()));
    if native && unique_machines(&artifacts).is_empty() {
        return Err("evidence names no native artifact machine".to_owned());
    }
    Ok(())
}

fn check_compiler(name: &str, producer: &str, artifacts: &[Value]) -> Result<(), String> {
    let mut signed = 0;
    for artifact in artifacts {
        let found: Vec<&str> = artifact["producers"]
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if found.is_empty() {
            continue;
        }
        signed += 1;
        if !found.contains(&producer) {
            return Err(format!(
                "artifact {} was not built by {name} ({producer:?}); its producers are {found:?}",
                artifact["path"]
            ));
        }
    }
    if signed == 0 {
        return Err(format!(
            "no artifact names its producer, so {name} ({producer:?}) is unproven"
        ));
    }
    Ok(())
}

fn git(source: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(source).args(args);
    let outcome = capture(command)?;
    if outcome.exit != 0 {
        return Err(format!("git {args:?} failed: {}", outcome.stderr));
    }
    Ok(outcome.stdout.trim().to_owned())
}

fn native(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 20];
    if std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_err()
    {
        return false;
    }
    let mach_o = [
        b"\xcf\xfa\xed\xfe".as_slice(),
        b"\xfe\xed\xfa\xcf",
        b"\xca\xfe\xba\xbe",
    ]
    .iter()
    .any(|magic| head.starts_with(magic));
    mach_o
        || head.starts_with(b"MZ")
        || (head.starts_with(b"\x7fELF") && dx_binfmt::machine(&head).is_some())
}

fn read_head(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(4096).read_to_end(&mut head))
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    Ok(head)
}

/// Whether Windows can load the image at another base: a PE with relocations stripped
/// (COFF Characteristics bit 0x1) cannot, and an AppContainer always relocates.
pub fn relocatable(head: &[u8]) -> bool {
    if !head.starts_with(b"MZ") {
        return true;
    }
    let Some(pe) = head
        .get(0x3c..0x40)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    else {
        return true;
    };
    if head.get(pe..pe + 4) != Some(b"PE\0\0".as_slice()) {
        return true;
    }
    head.get(pe + 22..pe + 24)
        .is_none_or(|characteristics| characteristics[0] & 0x1 == 0)
}

/// The Bazel binary `bazelisk --print_env` resolved: `BAZEL_REAL` when a wrapper is
/// configured, otherwise the directory bazelisk puts first on the `PATH` it prints.
pub fn bazel_real(stdout: &str) -> Result<PathBuf, String> {
    let value = |key: &str| {
        stdout.lines().find_map(|line| {
            let (name, value) = line.trim_end().split_once('=')?;
            Some(value)
                .filter(|value| name.eq_ignore_ascii_case(key) && !value.is_empty())
                .map(str::to_owned)
        })
    };
    if let Some(real) = value("BAZEL_REAL") {
        return Ok(PathBuf::from(real));
    }
    value("PATH")
        .and_then(|path| path.split(';').next().map(str::to_owned))
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(&dir).join(format!("bazel{}", std::env::consts::EXE_SUFFIX)))
        .ok_or_else(|| "bazelisk --print_env printed neither BAZEL_REAL nor PATH".to_owned())
}

fn find_bazelisk(chosen: Option<&str>, path: &str) -> Result<PathBuf, String> {
    if let Some(chosen) = chosen.filter(|chosen| !chosen.is_empty()) {
        let mut chosen = PathBuf::from(chosen);
        let exe = PathBuf::from(format!(
            "{}{}",
            chosen.display(),
            std::env::consts::EXE_SUFFIX
        ));
        if !std::env::consts::EXE_SUFFIX.is_empty() && chosen.extension().is_none() && exe.is_file()
        {
            chosen = exe;
        }
        if !native(&chosen) {
            return Err(format!(
                "DX_HERMETIC_BAZELISK {} is not a native bazelisk binary; a wrapper script would run outside the isolation",
                chosen.display()
            ));
        }
        return Ok(chosen);
    }
    for name in ["bazelisk", "bazel"] {
        for dir in std::env::split_paths(path).filter(|dir| !dir.as_os_str().is_empty()) {
            let candidate = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            if candidate.is_file() && native(&candidate) {
                return Ok(candidate);
            }
        }
    }
    Err("no native bazelisk: set DX_HERMETIC_BAZELISK or put bazelisk on PATH".to_owned())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|err| format!("cannot copy {} to {}: {err}", from.display(), to.display()))
}

/// Derives the consumer BUILD file from a repository one: rules_dx renamed, repository-only statements dropped.
pub fn consumer_build(text: &str, repo_name: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0i32;
    for line in text.lines() {
        let drop = if depth > 0 {
            true
        } else {
            (line.starts_with("package(") && line != "package(default_visibility = _PUBLIC)")
                || line.starts_with("exports_files(")
                || line.starts_with("real_source_target(")
                || (line.starts_with("load(") && line.contains("//quality:fixtures.bzl"))
        };
        if drop {
            depth += line.matches('(').count() as i32 - line.matches(')').count() as i32;
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("visibility = ") && trimmed.ends_with("],") {
            continue;
        }
        out.push(line.replace("@rules_dx//", &format!("@{repo_name}//")));
    }
    let mut text = String::new();
    for line in out {
        if line.is_empty() && (text.is_empty() || text.ends_with("\n\n")) {
            continue;
        }
        text.push_str(&line);
        text.push('\n');
    }
    format!("{}\n", text.trim_end())
}

#[cfg(windows)]
fn executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|err| format!("cannot mark {} executable: {err}", path.display()))
}

/// Builds the dx init arguments, adding the profile only when the manifest names one.
pub fn init_args<'a>(manifest: &'a Manifest, workspace: &'a str) -> Vec<&'a str> {
    let mut args = vec!["init", "--apply", "--workspace", workspace];
    if !manifest.profile.is_empty() {
        args.extend(["--profile", manifest.profile.as_str()]);
    }
    args
}

fn materialize_init(manifest: &Manifest, workspace: &Path, source: &Path) -> Result<(), String> {
    let workspace_text = workspace.to_string_lossy().into_owned();
    let init = dx_testing::run(&manifest.dx, &init_args(manifest, &workspace_text), &[])
        .map_err(|err| format!("cannot run dx init: {err}"))?;
    if !init.status.success() {
        return Err(format!("dx init failed: {}", init.combined()));
    }
    let snippet = std::fs::read_to_string(workspace.join("MODULE.bazel"))
        .map_err(|err| format!("dx init wrote no MODULE.bazel: {err}"))?;
    let fragment_path = manifest
        .module_fragment
        .as_ref()
        .ok_or("manifest needs a module_fragment")?;
    let fragment = std::fs::read_to_string(fragment_path)
        .map_err(|err| format!("cannot read {}: {err}", fragment_path.display()))?;
    let module = compose_module(
        &snippet,
        &manifest.module,
        &manifest.repo_name,
        &fragment,
        source,
    )?;
    let module = format!("{module}{}", family_modules(&manifest.families, source));
    // A floor search resolves candidate versions: DX_CONSUMER_OVERRIDES=module=version,...
    let module = match std::env::var(dx_floor::OVERRIDES_ENV) {
        Ok(spec) if !spec.trim().is_empty() => {
            dx_floor::apply_overrides(&module, &dx_floor::parse_pairs(&spec)?)?
        }
        _ => module,
    };
    write(&workspace.join("MODULE.bazel"), &module)?;
    rename_preset(workspace, &manifest.repo_name)?;
    copy(&manifest.bazelversion, &workspace.join(".bazelversion"))
}

/// Points the preset layers `dx init` wrote at the repo name the composed MODULE.bazel gives rules_dx.
pub fn rename_preset(workspace: &Path, repo_name: &str) -> Result<(), String> {
    let dir = workspace.join(".dx").join("bazelrc");
    let entries =
        std::fs::read_dir(&dir).map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|err| format!("cannot list {}: {err}", dir.display()))?
            .path();
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let renamed = text.replace("@rules_dx//", &format!("@{repo_name}//"));
        write(&path, &renamed)?;
    }
    Ok(())
}

/// Builds the consumer workspace with the public dx entry point and its own files.
pub fn materialize(
    manifest: &Manifest,
    workspace: &Path,
    source: &Path,
    ambient: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(workspace)
        .map_err(|err| format!("cannot create {}: {err}", workspace.display()))?;
    let repo = &manifest.repo_name;
    materialize_init(manifest, workspace, source)?;
    for (src, dest) in &manifest.files {
        if dest.ends_with("BUILD.bazel") {
            let text = std::fs::read_to_string(src)
                .map_err(|err| format!("cannot read {}: {err}", src.display()))?;
            write(&workspace.join(dest), &consumer_build(&text, repo))?;
        } else {
            copy(src, &workspace.join(dest))?;
        }
    }
    write(
        &workspace.join(LEAKS_PACKAGE).join("BUILD.bazel"),
        &leaks_build_host(ambient),
    )
}

fn host() -> Value {
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "kernel": kernel.trim(),
    })
}

/// Where a qualification runs. Windows gets a short root on the drive of `tmp`, because
/// bsdtar changes into each npm package directory and `SetCurrentDirectory` stops at 258
/// characters; the nested execroot under `TEST_TMPDIR` is too deep for that.
pub fn qualification_root_for(tmp: &Path, windows: bool) -> PathBuf {
    let text = tmp.to_string_lossy();
    let bytes = text.as_bytes();
    if !windows || bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return tmp.join("qualification");
    }
    let digest = sha256_hex(text.as_bytes());
    PathBuf::from(format!("{}\\q\\{}", &text[..2], &digest[..6]))
}

pub fn qualification_root(tmp: &Path) -> PathBuf {
    qualification_root_for(tmp, cfg!(windows))
}

/// The startup flags of every nested Bazel. On Windows the output base is a sibling of
/// `output_root` named by its first letter (`cold` gets `c`) instead of Bazel's 32-character
/// hash under it, for the same path length reason.
fn startup_flags_for(output_root: &Path, windows: bool) -> Vec<String> {
    let root = output_root.display().to_string();
    let mut flags = vec![
        "--nosystem_rc".to_owned(),
        format!("--output_user_root={root}"),
    ];
    if windows {
        let name = root.rfind(['\\', '/']).map_or(0, |at| at + 1);
        let end = root[name..]
            .chars()
            .next()
            .map_or(name, |c| name + c.len_utf8());
        flags.push(format!("--output_base={}", &root[..end]));
    }
    flags
}

fn startup_flags(output_root: &Path) -> Vec<String> {
    startup_flags_for(output_root, cfg!(windows))
}

/// The user cache dir of every Bazel and dx call on `output_root`. dx puts its disk cache
/// there, so raw probes and dx share one cache per root and `warm` stays cold to `cold`.
fn cache_home(output_root: &Path) -> PathBuf {
    let name = output_root
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    output_root.with_file_name(format!("{name}_cache"))
}

fn xdg_cache_home(output_root: &Path) -> (String, String) {
    (
        "XDG_CACHE_HOME".to_owned(),
        cache_home(output_root).to_string_lossy().into_owned(),
    )
}

/// Removes a qualification root, including Bazel's read-only trees.
pub fn remove_root(root: &Path) {
    dx_process::remove_tree(root);
}

// LCOV_EXCL_START - reason: runs only inside the isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
/// Confirms this host can create the namespaces the qualification needs.
pub fn preflight(isolation: &Isolation) -> Result<(), String> {
    let probe: &[&str] = if cfg!(windows) {
        &["cmd.exe", "/c", "exit 0"]
    } else if cfg!(target_os = "macos") {
        &["/usr/bin/true"]
    } else {
        &["/bin/true"]
    };
    let probe: Vec<String> = probe.iter().map(|arg| (*arg).to_owned()).collect();
    let outcome = capture(isolation.command(Net::Denied, &probe))?;
    if outcome.exit != 0 {
        return Err(format!(
            "namespace isolation is unavailable on this host (exit {}): {}; unprivileged user, mount and network namespaces are required, and Ubuntu 24.04 needs kernel.apparmor_restrict_unprivileged_userns=0",
            outcome.exit,
            outcome.stderr.trim()
        ));
    }
    Ok(())
}

/// Runs every probe of one qualification and returns its evidence.
pub fn qualify(manifest: Manifest, root: &Path, out: &Path) -> Value {
    let label = manifest.label.clone();
    let mut evidence = json!({
        "schema": SCHEMA,
        "qualification": label,
        "languages": manifest.languages,
        "host": host(),
        "result": "failed",
    });
    let mut session = None;
    let outcome = run_all(manifest, root, out, &mut evidence, &mut session);
    if let Some(session) = session {
        evidence["probes"] = json!(session.probes);
    }
    match outcome {
        Ok(()) => evidence["result"] = json!("qualified"),
        Err(err) => evidence["error"] = json!(err),
    }
    evidence
}

fn run_all(
    manifest: Manifest,
    root: &Path,
    out: &Path,
    evidence: &mut Value,
    slot: &mut Option<Session>,
) -> Result<(), String> {
    let source = std::fs::canonicalize(&manifest.module_file)
        .map(strip_verbatim)
        .map_err(|err| format!("cannot resolve the rules_dx source tree: {err}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or("rules_dx MODULE.bazel has no parent")?;
    let commit = git(&source, &["rev-parse", "HEAD"])?;
    let changes = git(&source, &["status", "--porcelain"])?;
    evidence["rules_dx"] = json!({"commit": commit, "dirty": !changes.is_empty(), "changes": changes, "source": source});
    let found = find_bazelisk(
        std::env::var("DX_HERMETIC_BAZELISK").ok().as_deref(),
        &std::env::var("PATH").unwrap_or_default(),
    )?;
    std::fs::create_dir_all(root.join("bin"))
        .map_err(|err| format!("cannot create {}: {err}", root.display()))?;
    let bazelisk = root
        .join("bin")
        .join(format!("bazelisk{}", std::env::consts::EXE_SUFFIX));
    copy(&found, &bazelisk)?;
    executable(&bazelisk)?;
    let shim_dir = root.join("bin").join("shims");
    let shim = root.join("bin").join(SHIM_NAME);
    write(&shim, SHIM)?;
    executable(&shim)?;
    let real_home = if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        real_ids()
            .ok()
            .and_then(|(uid, _)| passwd_home(&passwd, uid))
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    };
    let keep = vec![root.to_path_buf(), out.to_path_buf(), source.clone()];
    let host = HostPaths::detect();
    let isolation = Isolation::plan_with(&host, &shim, &keep, real_home.as_deref())?;
    let ambient = ambient_tools(&host.tool_dirs, AMBIENT_TOOLS)
        .into_iter()
        .find(|(_, path)| {
            isolation.tools.contains(path)
                || (cfg!(windows) && isolation.sdk_dirs.iter().any(|dir| covers(dir, path)))
        })
        .map(|(name, _)| name)
        .ok_or("no ambient compiler or interpreter on this host for the ambient-tool probe")?;
    if cfg!(windows) {
        write(&shim_dir.join(format!("{ambient}.cmd")), SHIM)?;
    }
    let path = action_path(std::env::var("SystemRoot").ok().as_deref(), Some(&shim_dir));
    let bash = if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        let program_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| format!(r"{drive}\Program Files"));
        let candidates = [
            PathBuf::from(format!(r"{drive}\msys64\usr\bin\bash.exe")),
            PathBuf::from(format!(r"{program_files}\Git\usr\bin\bash.exe")),
        ];
        let bazel_sh = std::env::var_os("BAZEL_SH").map(PathBuf::from);
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        Some(
            windows_bash(bazel_sh.as_deref(), &system_root, &candidates)
                .ok_or("no MSYS bash: set BAZEL_SH")?,
        )
    } else {
        None
    };
    evidence["isolation"] = json!({
        "network": if cfg!(target_os = "macos") {
            "sandbox-exec profile, loopback only"
        } else if cfg!(windows) {
            "AppContainer without network capabilities"
        } else {
            "fresh network namespace with loopback only"
        },
        "path": path,
        "masked_tools": isolation.tools,
        "masked_sdk_dirs": isolation.sdk_dirs,
        "masked_home": isolation.home_dirs.iter().chain(&isolation.home_files).collect::<Vec<_>>(),
        "ambient_probe_tool": ambient,
        "bash": bash,
    });
    preflight(&isolation)?;
    let workspace = root.join(&manifest.workspace_dir);
    evidence["consumer"] = json!({
        "module": manifest.module,
        "repo_name": manifest.repo_name,
        "workspace": workspace,
        "patterns": manifest.patterns,
    });
    if let Some((name, producer)) = &manifest.compiler {
        evidence["compiler"] = json!({"name": name, "producer": producer});
    }
    let session = slot.insert(Session {
        manifest,
        root: root.to_path_buf(),
        workspace,
        out: out.to_path_buf(),
        source,
        bazel: bazelisk.clone(),
        bazelisk,
        bazelisk_home: root.join("bazelisk"),
        isolation,
        probes: Vec::new(),
        bash,
    });
    let clean_home = root.join("home");
    let planted_home = root.join("home-planted");
    std::fs::create_dir_all(&clean_home)
        .map_err(|err| format!("cannot create {}: {err}", clean_home.display()))?;
    materialize(
        &session.manifest,
        &session.workspace,
        &session.source,
        &ambient,
    )?;
    if cfg!(windows) {
        // An AppContainer relocates every image, and bazelisk's Windows release has its
        // relocations stripped, so resolve Bazel here and run that inside instead.
        let printed = capture({
            let mut command = Command::new(&session.bazelisk);
            command
                .env_clear()
                .envs(session.windows_env(&clean_home, &path))
                .current_dir(&session.workspace)
                .arg("--print_env");
            command
        })?;
        if printed.exit != 0 {
            return Err(format!("bazelisk --print_env failed: {}", printed.stderr));
        }
        let bazel = bazel_real(&printed.stdout)?;
        if !bazel.is_file() {
            return Err(format!(
                "bazelisk --print_env resolved {}, which does not exist",
                bazel.display()
            ));
        }
        let head = read_head(&bazel)?;
        if !relocatable(&head) {
            return Err(format!(
                "{} has no relocation table; an AppContainer must relocate it (error 623)",
                bazel.display()
            ));
        }
        session.bazel = bazel;
    }
    write(&planted_home.join(".bazelrc"), HOME_RC)?;
    let repos = root.join("repos");
    let empty_repos = root.join("repos-empty");
    std::fs::create_dir_all(&empty_repos)
        .map_err(|err| format!("cannot create {}: {err}", empty_repos.display()))?;
    let cold = root.join("cold");
    let warm = root.join("warm");
    let miss = root.join("miss");
    let patterns = session.manifest.patterns.clone();
    let coverage = session.manifest.coverage.clone();
    let failures = session.manifest.failures.clone();
    let leak = |name: &str| vec![format!("//{LEAKS_PACKAGE}:{name}")];
    if session.manifest.generate {
        session.run_dx(
            "dx_generate",
            Net::Allowed,
            "generate",
            &patterns,
            &clean_home,
            &cold,
            &repos,
        )?;
    }
    session.run(Probe {
        name: "fetch".to_owned(),
        net: Net::Allowed,
        home: &clean_home,
        output_root: &cold,
        repos: &repos,
        command: "build",
        targets: patterns.clone(),
        extra: vec!["--nobuild".to_owned()],
        expect: Expect::Pass,
    })?;
    if !coverage.is_empty() {
        let mut fetched = coverage.clone();
        fetched.push(format!(
            "@{}//dx:generate_check",
            session.manifest.repo_name
        ));
        session.run(Probe {
            name: "fetch_coverage".to_owned(),
            net: Net::Allowed,
            home: &clean_home,
            output_root: &cold,
            repos: &repos,
            command: "build",
            targets: fetched,
            extra: vec![
                "--nobuild".to_owned(),
                "--collect_code_coverage".to_owned(),
                "--combined_report=lcov".to_owned(),
            ],
            expect: Expect::Pass,
        })?;
    }
    let version = capture({
        let mut command = Command::new(&session.bazelisk);
        command
            .env_clear()
            .env("HOME", &clean_home)
            .env("PATH", &path)
            .env("BAZELISK_HOME", &session.bazelisk_home)
            .envs(session.windows_env(&clean_home, &path))
            .current_dir(&session.workspace)
            .arg("--version");
        command
    })?;
    evidence["bazel"] = json!({
        "version": version.stdout.trim(),
        "bazelisk": found,
        "isolated": session.bazel,
    });
    session.run(Probe {
        name: "hermetic_test".to_owned(),
        net: Net::Denied,
        home: &clean_home,
        output_root: &cold,
        repos: &repos,
        command: "test",
        targets: session.hermetic_test_targets(),
        extra: vec!["--test_output=errors".to_owned()],
        expect: Expect::Pass,
    })?;
    let listed = session.run(Probe {
        name: "artifacts".to_owned(),
        net: Net::Denied,
        home: &clean_home,
        output_root: &cold,
        repos: &repos,
        command: "cquery",
        targets: vec![built_query(&patterns)],
        extra: vec!["--output=files".to_owned()],
        expect: Expect::Pass,
    })?;
    let built = artifacts(&session.workspace, &listed.stdout)?;
    evidence["target"] = json!({"machines": unique_machines(&built)});
    evidence["exec"] = json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH});
    evidence["artifacts"] = json!(built);
    let (ambient_marker, network_marker, home_marker) = markers(std::env::consts::OS);
    for (name, marker) in [
        ("ambient_tool", ambient_marker),
        ("network", network_marker),
        ("home_config", home_marker),
    ] {
        session.run(Probe {
            name: name.to_owned(),
            net: Net::Denied,
            home: &clean_home,
            output_root: &cold,
            repos: &repos,
            command: "build",
            targets: leak(name),
            extra: Vec::new(),
            expect: Expect::Fail(marker.to_owned()),
        })?;
    }
    session.run(Probe {
        name: "home_config_control".to_owned(),
        net: Net::Denied,
        home: &planted_home,
        output_root: &cold,
        repos: &repos,
        command: "build",
        targets: leak("home_config"),
        extra: Vec::new(),
        expect: Expect::Pass,
    })?;
    session.run(Probe {
        name: "warm_cache".to_owned(),
        net: Net::Denied,
        home: &clean_home,
        output_root: &warm,
        repos: &repos,
        command: "test",
        targets: patterns.clone(),
        extra: vec![
            "--repository_disable_download".to_owned(),
            "--test_output=errors".to_owned(),
        ],
        expect: Expect::Pass,
    })?;
    if !coverage.is_empty() {
        session.run(Probe {
            name: "coverage".to_owned(),
            net: Net::Denied,
            home: &clean_home,
            output_root: &warm,
            repos: &repos,
            command: "coverage",
            targets: coverage,
            extra: vec![
                "--combined_report=lcov".to_owned(),
                "--test_output=errors".to_owned(),
            ],
            expect: Expect::Pass,
        })?;
        let report_path = session
            .workspace
            .join("bazel-out/_coverage/_coverage_report.dat");
        let report = std::fs::read_to_string(&report_path)
            .map_err(|err| format!("cannot read {}: {err}", report_path.display()))?;
        write(&out.join("coverage.lcov"), &report)?;
        let sources = coverage_sources(&report)?;
        evidence["coverage"] = json!({
            "report": "coverage.lcov",
            "sources": sources
                .iter()
                .map(|source| json!({
                    "path": source.path,
                    "lines_found": source.lines_found,
                    "lines_hit": source.lines_hit,
                }))
                .collect::<Vec<_>>(),
        });
        check_coverage(&sources, &session.manifest)?;
        let coverage_targets = session.manifest.coverage.clone();
        for (name, command, root) in [
            ("dx_generate_check", "generate", &cold),
            ("dx_coverage", "coverage", &warm),
        ] {
            session.run_dx(
                name,
                Net::Denied,
                command,
                &coverage_targets,
                &clean_home,
                root,
                &repos,
            )?;
        }
    }
    if !session.manifest.lint_findings.is_empty() {
        let lint = session.run_dx(
            "dx_lint",
            Net::Denied,
            "lint",
            &["//...".to_owned()],
            &clean_home,
            &warm,
            &repos,
        )?;
        let counts = lint_counts(&lint.stdout);
        if lint.exit == 1 && counts.is_empty() {
            return Err("dx lint exited 1 without reporting a finding".to_owned());
        }
        evidence["lint"] = json!(counts);
    }
    for (label, marker) in failures {
        session.run(Probe {
            name: failure_probe_name(&label),
            net: Net::Denied,
            home: &clean_home,
            output_root: &warm,
            repos: &repos,
            command: "build",
            targets: vec![label],
            extra: Vec::new(),
            expect: Expect::Fail(marker),
        })?;
    }
    session.run(Probe {
        name: "cache_miss".to_owned(),
        net: Net::Denied,
        home: &clean_home,
        output_root: &miss,
        repos: &empty_repos,
        command: "build",
        targets: patterns,
        extra: Vec::new(),
        expect: Expect::Fail(miss_marker(std::env::consts::OS).to_owned()),
    })?;
    Ok(())
}

// LCOV_EXCL_STOP - reason: end isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests;
