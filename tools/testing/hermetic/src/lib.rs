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
use std::process::Command;
use std::time::Instant;

use serde_json::{json, Value};

/// Returns the lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(data))
}

pub const SCHEMA: &str = "dx.hermetic.v1";

pub const AMBIENT_TOOLS: &[&str] = &[
    "cc", "gcc", "c++", "g++", "cpp", "clang", "clang++", "ld", "ld.lld", "ld.gold", "as", "ar",
    "ranlib", "nm", "strip", "objcopy", "objdump", "python", "python3", "perl", "ruby", "gem",
    "node", "npm", "npx", "yarn", "pnpm", "java", "javac", "go", "rustc", "cargo", "rustup",
    "dotnet", "mono", "swift", "swiftc", "kotlinc", "scalac", "pip", "pip3", "make", "cmake",
    "ninja", "curl", "wget", "git", "tar", "unzip", "xz", "bzip2", "zstd", "7z", "7za", "nvcc",
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
            tool_dirs: TOOL_DIRS.iter().map(PathBuf::from).collect(),
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

pub const SHIM: &str = "#!/bin/sh\necho \"dx-hermetic: undeclared ambient tool $0\" >&2\nexit 97\n";

pub const AMBIENT_MARKER: &str = "dx-hermetic: undeclared ambient tool";

pub const NETWORK_MARKER: &str = "Network is unreachable";

pub const HOME_MARKER: &str = "dx-hermetic: home config absent";

pub const MISS_MARKER: &str = "Unknown host";

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
    pub rust_target_triples: Vec<String>,
    pub rust_extra_rustc_flags: Vec<String>,
    pub workspace_dir: String,
    pub patterns: Vec<String>,
    pub failures: Vec<(String, String)>,
    pub coverage: Vec<String>,
    pub covered: Vec<String>,
    pub files: Vec<(PathBuf, String)>,
    pub module_fragment: Option<PathBuf>,
    pub starter: Option<String>,
    pub fragment_before_toolchains: bool,
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
    let starter = value["starter"]
        .as_str()
        .filter(|starter| !starter.is_empty())
        .map(str::to_owned);
    let fragment = value["module_fragment"].as_str().unwrap_or("");
    let fragment_before_toolchains = value["fragment_before_toolchains"]
        .as_bool()
        .unwrap_or(false);
    let rust_target_triples = strings(&value, "rust_target_triples")?;
    let rust_extra_rustc_flags = strings(&value, "rust_extra_rustc_flags")?;
    let repo_name = string(&value, "repo_name")?;
    let module_fragment = if fragment.is_empty() {
        None
    } else {
        Some(base.join(fragment))
    };
    if starter.is_some() {
        if repo_name != "rules_dx" {
            return Err("a starter manifest must name rules_dx as repo_name".to_owned());
        }
        if fragment_before_toolchains
            || !rust_target_triples.is_empty()
            || !rust_extra_rustc_flags.is_empty()
        {
            return Err(
                "a starter manifest cannot edit the dx init snippet (fragment_before_toolchains, rust_target_triples, rust_extra_rustc_flags)"
                    .to_owned(),
            );
        }
    } else if module_fragment.is_none() {
        return Err("manifest needs a module_fragment unless it names a starter".to_owned());
    }
    Ok(Manifest {
        label: string(&value, "label")?,
        languages,
        module: string(&value, "module")?,
        repo_name,
        rust_target_triples,
        rust_extra_rustc_flags,
        workspace_dir: checked_dest(&string(&value, "workspace_dir")?)?,
        patterns,
        failures,
        coverage,
        covered,
        files,
        module_fragment,
        starter,
        fragment_before_toolchains,
        dx: base.join(string(&value, "dx")?),
        module_file: base.join(string(&value, "module_file")?),
        bazelversion: base.join(string(&value, "bazelversion")?),
    })
}

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Adds the list attribute `name = [values]` to the one `rust.toolchain` in the `dx init` snippet.
pub fn add_rust_toolchain_list(
    snippet: &str,
    name: &str,
    values: &[String],
) -> Result<String, String> {
    if values.is_empty() {
        return Ok(snippet.to_owned());
    }
    let needle = "rust.toolchain(\n";
    if snippet.matches("rust.toolchain(").count() != 1 || !snippet.contains(needle) {
        return Err(format!(
            "dx init snippet must hold exactly one rust.toolchain: {snippet}"
        ));
    }
    let quoted: Vec<String> = values.iter().map(|value| starlark_string(value)).collect();
    Ok(snippet.replacen(
        needle,
        &format!("{needle}    {name} = [{}],\n", quoted.join(", ")),
        1,
    ))
}

/// Inserts `fragment` before the first `register_toolchains(` line of `setup`.
fn insert_before_toolchains(setup: &str, fragment: &str) -> Result<String, String> {
    let mut at = None;
    let mut offset = 0;
    for line in setup.split_inclusive('\n') {
        if line.starts_with("register_toolchains(") {
            at = Some(offset);
            break;
        }
        offset += line.len();
    }
    let at = at.ok_or_else(|| {
        format!("dx init snippet has no register_toolchains to put the fragment before: {setup}")
    })?;
    Ok(format!(
        "{}{}\n\n{}",
        &setup[..at],
        fragment.trim_end(),
        &setup[at..]
    ))
}

fn rules_dx_override(rules_dx: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&rules_dx.to_string_lossy()),
    )
}

/// Points the MODULE.bazel `dx new` wrote at this checkout, keeping its own toolchain wiring.
pub fn starter_module(
    text: &str,
    fragment: Option<&str>,
    rules_dx: &Path,
) -> Result<String, String> {
    if text.matches("bazel_dep(name = \"rules_dx\", ").count() != 1 {
        return Err(format!(
            "dx new MODULE.bazel must declare rules_dx exactly once: {text}"
        ));
    }
    if text.contains("local_path_override(") {
        return Err("dx new MODULE.bazel already carries a local_path_override".to_owned());
    }
    let mut module = format!("{}\n", text.trim_end());
    if let Some(fragment) = fragment {
        module.push_str(&format!("{}\n", fragment.trim_end()));
    }
    module.push_str(&rules_dx_override(rules_dx));
    Ok(module)
}

/// Builds the consumer MODULE.bazel from the `dx init` snippet.
///
/// The fragment is appended, or with `before_toolchains` placed ahead of the snippet's first
/// toolchain registration so its own registrations resolve first.
pub fn compose_module(
    snippet: &str,
    module: &str,
    repo_name: &str,
    fragment: &str,
    before_toolchains: bool,
    rules_dx: &Path,
) -> Result<String, String> {
    let needle = "bazel_dep(name = \"rules_dx\", ";
    if snippet.matches(needle).count() != 1 {
        return Err(format!(
            "dx init snippet must declare rules_dx exactly once: {snippet}"
        ));
    }
    let setup = if repo_name == "rules_dx" {
        snippet.to_owned()
    } else {
        snippet.replacen(
            needle,
            &format!("{needle}repo_name = {}, ", starlark_string(repo_name)),
            1,
        )
    };
    let body = if before_toolchains {
        insert_before_toolchains(&setup, fragment)?
            .trim_end()
            .to_owned()
    } else {
        format!("{}\n{}", setup.trim_end(), fragment.trim_end())
    };
    Ok(format!(
        "module(name = {})\n\n{}\n{}",
        starlark_string(module),
        body,
        rules_dx_override(rules_dx),
    ))
}

/// Returns the leak package that proves each isolation boundary.
pub fn leaks_build(tool: &str) -> String {
    format!(
        "genrule(\n    name = \"ambient_tool\",\n    outs = [\"ambient_tool.txt\"],\n    cmd = \"{tool} --version > $@\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"home_config\",\n    outs = [\"home_config.txt\"],\n    cmd = \"test \\\"$${{DX_HOME_CONFIG:-absent}}\\\" = present || {{ echo dx-hermetic: home config $${{DX_HOME_CONFIG:-absent}} >&2; exit 1; }}; echo present > $@\",\n    tags = [\"manual\"],\n)\n\ngenrule(\n    name = \"network\",\n    outs = [\"network.txt\"],\n    cmd = \"exec 3<>/dev/tcp/1.1.1.1/443 && echo connected > $@\",\n    tags = [\n        \"manual\",\n        \"requires-network\",\n    ],\n)\n"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    Pass,
    Fail(String),
}

/// Reports whether one probe ended the way the qualification requires.
pub fn verdict(expect: &Expect, exit: i32, log: &str) -> Result<(), String> {
    match expect {
        Expect::Pass if exit == 0 => Ok(()),
        Expect::Pass => Err(format!("exited {exit}, want 0")),
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
                let Ok(real) = std::fs::canonicalize(dir.join(&entry)) else {
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
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|err| format!("cannot read /proc/self/status: {err}"))?;
        let uid = status_id(&status, "Uid:").ok_or("no Uid in /proc/self/status")?;
        let gid = status_id(&status, "Gid:").ok_or("no Gid in /proc/self/status")?;
        let ip = host
            .ip_candidates
            .iter()
            .find(|path| path.is_file())
            .cloned()
            .ok_or("namespace isolation needs ip from iproute2 to raise loopback")?;
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

/// Builds the isolated `dx new` command line that scaffolds the starter into `workspace_dir`.
pub fn scaffold_argv(
    dx: &Path,
    starter: &str,
    workspace_dir: &str,
    root: &Path,
    home: &Path,
) -> Vec<String> {
    vec![
        "/usr/bin/env".to_owned(),
        "-i".to_owned(),
        format!("HOME={}", home.display()),
        format!("PATH={ACTION_PATH}"),
        "/bin/sh".to_owned(),
        "-c".to_owned(),
        "cd \"$0\" && exec \"$@\"".to_owned(),
        root.to_string_lossy().into_owned(),
        dx.to_string_lossy().into_owned(),
        "new".to_owned(),
        "--apply".to_owned(),
        starter.to_owned(),
        workspace_dir.to_owned(),
        "--workspace".to_owned(),
        root.to_string_lossy().into_owned(),
    ]
}

pub struct Session {
    pub manifest: Manifest,
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub out: PathBuf,
    pub source: PathBuf,
    pub bazelisk: PathBuf,
    pub bazelisk_home: PathBuf,
    pub isolation: Isolation,
    pub probes: Vec<Value>,
}

impl Session {
    fn bazel_argv(&self, probe: &Probe) -> Vec<String> {
        let mut argv = vec![
            "/usr/bin/env".to_owned(),
            "-i".to_owned(),
            format!("HOME={}", probe.home.display()),
            format!("PATH={ACTION_PATH}"),
            format!("BAZELISK_HOME={}", self.bazelisk_home.display()),
            "/bin/sh".to_owned(),
            "-c".to_owned(),
            "cd \"$0\" && \"$@\"; rc=$?; \"$1\" \"$2\" \"$3\" \"$4\" shutdown >/dev/null 2>&1; exit $rc"
                .to_owned(),
            self.workspace.to_string_lossy().into_owned(),
            self.bazelisk.to_string_lossy().into_owned(),
            "--nosystem_rc".to_owned(),
            format!("--output_user_root={}", probe.output_root.display()),
            "--max_idle_secs=60".to_owned(),
            probe.command.to_owned(),
            format!("--repository_cache={}", probe.repos.display()),
        ];
        argv.extend(BUILD_FLAGS.iter().map(|flag| (*flag).to_owned()));
        argv.extend(probe.extra.iter().cloned());
        argv.push("--".to_owned());
        argv.extend(probe.targets.iter().cloned());
        argv
    }

    // LCOV_EXCL_START - reason: runs only inside the isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
    /// Runs one probe, logs it and records its verdict.
    pub fn run(&mut self, probe: Probe) -> Result<Outcome, String> {
        let argv = self.bazel_argv(&probe);
        let recorded = argv[9..].to_vec();
        self.execute(
            &probe.name,
            probe.net,
            probe.home,
            &argv,
            recorded,
            &probe.expect,
        )
    }

    /// Scaffolds the starter inside the isolation with the network denied.
    pub fn scaffold(&mut self, starter: &str, home: &Path) -> Result<Outcome, String> {
        let argv = scaffold_argv(
            &self.manifest.dx,
            starter,
            &self.manifest.workspace_dir,
            &self.root,
            home,
        );
        let recorded = argv.clone();
        self.execute(
            "scaffold",
            Net::Denied,
            home,
            &argv,
            recorded,
            &Expect::Pass,
        )
    }

    fn execute(
        &mut self,
        name: &str,
        net: Net,
        home: &Path,
        argv: &[String],
        recorded: Vec<String>,
        expect: &Expect,
    ) -> Result<Outcome, String> {
        let started = Instant::now();
        let outcome = capture(self.isolation.command(net, argv))?;
        let seconds = started.elapsed().as_secs();
        let log = format!("{}{}", outcome.stdout, outcome.stderr);
        let logs = self.out.join("logs");
        std::fs::create_dir_all(&logs).map_err(|err| format!("cannot create logs: {err}"))?;
        let log_path = logs.join(format!("{name}.log"));
        std::fs::write(&log_path, &log)
            .map_err(|err| format!("cannot write {}: {err}", log_path.display()))?;
        let checked = verdict(expect, outcome.exit, &log);
        let (expected, marker) = match expect {
            Expect::Pass => ("pass", ""),
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
        checked.map_err(|err| format!("probe {name} {err}; see {}", log_path.display()))?;
        Ok(outcome)
    }
    // LCOV_EXCL_STOP - reason: end isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
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
        }));
    }
    if out.is_empty() {
        return Err("cquery listed no consumer artifacts".to_owned());
    }
    Ok(out)
}

fn probe_expectation(name: &str) -> (&'static str, &'static str) {
    match name {
        "ambient_tool" => ("fail", AMBIENT_MARKER),
        "network" => ("fail", NETWORK_MARKER),
        "home_config" => ("fail", HOME_MARKER),
        "cache_miss" => ("fail", MISS_MARKER),
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

fn required_probes(manifest: &Manifest) -> Vec<(String, &str, &str, Net)> {
    let mut required: Vec<(String, &str, &str, Net)> = REQUIRED_PROBES
        .iter()
        .map(|name| {
            let (expect, marker) = probe_expectation(name);
            let net = if *name == "fetch" {
                Net::Allowed
            } else {
                Net::Denied
            };
            ((*name).to_owned(), expect, marker, net)
        })
        .collect();
    if !manifest.coverage.is_empty() {
        required.push(("fetch_coverage".to_owned(), "pass", "", Net::Allowed));
        required.push(("coverage".to_owned(), "pass", "", Net::Denied));
    }
    if manifest.starter.is_some() {
        required.push(("scaffold".to_owned(), "pass", "", Net::Denied));
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
    for (name, expect, marker, net) in required_probes(manifest) {
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
    let native = languages
        .iter()
        .any(|language| NATIVE_LANGUAGES.contains(&language.as_str()));
    if native && unique_machines(&artifacts).is_empty() {
        return Err("evidence names no native artifact machine".to_owned());
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
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && head.starts_with(b"\x7fELF")
        && dx_binfmt::machine(&head).is_some()
}

fn find_bazelisk(chosen: Option<&str>, path: &str) -> Result<PathBuf, String> {
    if let Some(chosen) = chosen.filter(|chosen| !chosen.is_empty()) {
        let chosen = PathBuf::from(chosen);
        if !native(&chosen) {
            return Err(format!(
                "DX_HERMETIC_BAZELISK {} is not a native bazelisk binary; a wrapper script would run outside the isolation",
                chosen.display()
            ));
        }
        return Ok(chosen);
    }
    for name in ["bazelisk", "bazel"] {
        for dir in path.split(':').filter(|dir| !dir.is_empty()) {
            let candidate = Path::new(dir).join(name);
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

fn executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|err| format!("cannot mark {} executable: {err}", path.display()))
}

fn materialize_init(manifest: &Manifest, workspace: &Path, source: &Path) -> Result<(), String> {
    let workspace_text = workspace.to_string_lossy().into_owned();
    let init = dx_testing::run(
        &manifest.dx,
        &[
            "init",
            "--apply",
            &manifest.module,
            "--workspace",
            &workspace_text,
        ],
        &[],
    )
    .map_err(|err| format!("cannot run dx init: {err}"))?;
    if !init.status.success() {
        return Err(format!("dx init failed: {}", init.combined()));
    }
    let snippet = std::fs::read_to_string(workspace.join("MODULE.bazel.snippet"))
        .map_err(|err| format!("dx init wrote no MODULE.bazel.snippet: {err}"))?;
    let fragment_path = manifest
        .module_fragment
        .as_ref()
        .ok_or("manifest needs a module_fragment")?;
    let fragment = std::fs::read_to_string(fragment_path)
        .map_err(|err| format!("cannot read {}: {err}", fragment_path.display()))?;
    let snippet = add_rust_toolchain_list(
        &snippet,
        "extra_target_triples",
        &manifest.rust_target_triples,
    )?;
    let snippet = add_rust_toolchain_list(
        &snippet,
        "extra_rustc_flags",
        &manifest.rust_extra_rustc_flags,
    )?;
    let module = compose_module(
        &snippet,
        &manifest.module,
        &manifest.repo_name,
        &fragment,
        manifest.fragment_before_toolchains,
        source,
    )?;
    // A floor search resolves candidate versions: DX_CONSUMER_OVERRIDES=module=version,...
    let module = match std::env::var(dx_floor::OVERRIDES_ENV) {
        Ok(spec) if !spec.trim().is_empty() => {
            dx_floor::apply_overrides(&module, &dx_floor::parse_pairs(&spec)?)?
        }
        _ => module,
    };
    write(&workspace.join("MODULE.bazel"), &module)?;
    copy(&manifest.bazelversion, &workspace.join(".bazelversion"))
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
    if manifest.starter.is_some() {
        let path = workspace.join("MODULE.bazel");
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("dx new wrote no MODULE.bazel: {err}"))?;
        let fragment = match &manifest.module_fragment {
            Some(path) => Some(
                std::fs::read_to_string(path)
                    .map_err(|err| format!("cannot read {}: {err}", path.display()))?,
            ),
            None => None,
        };
        write(&path, &starter_module(&text, fragment.as_deref(), source)?)?;
        for (_, dest) in &manifest.files {
            if workspace.join(dest).exists() {
                return Err(format!("{dest} would overwrite dx new output"));
            }
        }
    } else {
        materialize_init(manifest, workspace, source)?;
    }
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
        &leaks_build(ambient),
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

/// Removes a qualification root, including Bazel's read-only trees.
pub fn remove_root(root: &Path) {
    dx_process::remove_tree(root);
}

// LCOV_EXCL_START - reason: runs only inside the isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md
/// Confirms this host can create the namespaces the qualification needs.
pub fn preflight(isolation: &Isolation) -> Result<(), String> {
    let outcome = capture(isolation.command(Net::Denied, &["/bin/true".to_owned()]))?;
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
    let bazelisk = root.join("bin").join("bazelisk");
    copy(&found, &bazelisk)?;
    executable(&bazelisk)?;
    let shim = root.join("bin").join("ambient-tool");
    write(&shim, SHIM)?;
    executable(&shim)?;
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let real_home = status_id(&status, "Uid:").and_then(|uid| passwd_home(&passwd, uid));
    let keep = vec![root.to_path_buf(), out.to_path_buf(), source.clone()];
    let host = HostPaths::detect();
    let isolation = Isolation::plan_with(&host, &shim, &keep, real_home.as_deref())?;
    let ambient = ambient_tools(&host.tool_dirs, AMBIENT_TOOLS)
        .into_iter()
        .find(|(_, path)| isolation.tools.contains(path))
        .map(|(name, _)| name)
        .ok_or("no ambient compiler or interpreter on this host for the ambient-tool probe")?;
    evidence["isolation"] = json!({
        "network": "fresh network namespace with loopback only",
        "path": ACTION_PATH,
        "masked_tools": isolation.tools,
        "masked_sdk_dirs": isolation.sdk_dirs,
        "masked_home": isolation.home_dirs.iter().chain(&isolation.home_files).collect::<Vec<_>>(),
        "ambient_probe_tool": ambient,
    });
    preflight(&isolation)?;
    let workspace = root.join(&manifest.workspace_dir);
    evidence["consumer"] = json!({
        "module": manifest.module,
        "repo_name": manifest.repo_name,
        "workspace": workspace,
        "patterns": manifest.patterns,
    });
    let session = slot.insert(Session {
        manifest,
        root: root.to_path_buf(),
        workspace,
        out: out.to_path_buf(),
        source,
        bazelisk,
        bazelisk_home: root.join("bazelisk"),
        isolation,
        probes: Vec::new(),
    });
    let clean_home = root.join("home");
    let planted_home = root.join("home-planted");
    std::fs::create_dir_all(&clean_home)
        .map_err(|err| format!("cannot create {}: {err}", clean_home.display()))?;
    if let Some(starter) = session.manifest.starter.clone() {
        evidence["consumer"]["starter"] = json!(starter);
        session.scaffold(&starter, &clean_home)?;
    }
    materialize(
        &session.manifest,
        &session.workspace,
        &session.source,
        &ambient,
    )?;
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
        session.run(Probe {
            name: "fetch_coverage".to_owned(),
            net: Net::Allowed,
            home: &clean_home,
            output_root: &cold,
            repos: &repos,
            command: "build",
            targets: coverage.clone(),
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
            .env("PATH", ACTION_PATH)
            .env("BAZELISK_HOME", &session.bazelisk_home)
            .current_dir(&session.workspace)
            .arg("--version");
        command
    })?;
    evidence["bazel"] = json!({
        "version": version.stdout.trim(),
        "bazelisk": found,
    });
    session.run(Probe {
        name: "hermetic_test".to_owned(),
        net: Net::Denied,
        home: &clean_home,
        output_root: &cold,
        repos: &repos,
        command: "test",
        targets: patterns.clone(),
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
    for (name, marker) in [
        ("ambient_tool", AMBIENT_MARKER),
        ("network", NETWORK_MARKER),
        ("home_config", HOME_MARKER),
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
        expect: Expect::Fail(MISS_MARKER.to_owned()),
    })?;
    Ok(())
}

// LCOV_EXCL_STOP - reason: end isolated qualification, issue: 1758, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests;
