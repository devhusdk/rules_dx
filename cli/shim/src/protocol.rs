use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const SHIM_VERSION: &str = "0.0.0";
pub const DEFAULT_RELEASE_BASE: &str = "https://github.com/devhusdk/rules_dx/releases/download";

pub const VERSION_FILE: &str = ".dx/pins";
pub const EXIT_USAGE: i32 = dx_error::EXIT_USAGE;
pub const EXIT_FAILURE: i32 = dx_error::EXIT_FAILURE;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchError {
    pub code: i32,
    pub message: String,
}

impl LaunchError {
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_FAILURE,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_USAGE,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dx: {}", self.message)
    }
}

impl std::error::Error for LaunchError {}
#[must_use]
pub fn version_is_valid(version: &str) -> bool {
    !version.is_empty() && semver::Version::parse(version).is_ok()
}
fn without_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn read_optional(path: &Path) -> Result<Option<String>, LaunchError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(LaunchError::failure(format!(
            "cannot read {}: {e}",
            path.display()
        ))),
    }
}

#[must_use]
pub fn pinned_hash(text: &str, version: &str, asset: &str) -> Option<String> {
    without_bom(text).lines().find_map(|line| {
        let fields: Vec<&str> = line.trim_end_matches('\r').split_whitespace().collect();
        let [line_version, hash, line_asset] = fields[..] else {
            return None;
        };
        let is_hash = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        (line_version == version && line_asset == asset && is_hash).then(|| hash.to_owned())
    })
}

#[must_use]
pub fn module_version(text: &str) -> Option<String> {
    let text = without_bom(text);
    let names_rules_dx = text.lines().any(|line| {
        line.split_once("module_name")
            .and_then(|(_, rest)| rest.trim_start().strip_prefix('='))
            .is_some_and(|rest| rest.trim_start().starts_with("\"rules_dx\""))
    });
    if names_rules_dx {
        return None;
    }
    let mut deps = text.lines().filter_map(|line| {
        let rest = line.strip_prefix("bazel_dep(name = \"rules_dx\", version = \"")?;
        let (version, tail) = rest.split_once('"')?;
        (tail.strip_prefix(')')?.trim().is_empty()).then_some(version)
    });
    let version = deps.next()?;
    let plain = !version.is_empty()
        && !version.starts_with('.')
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'));
    (deps.next().is_none() && plain).then(|| version.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSource {
    Env,
    ModuleBazel,
    Bazel,
}

impl std::fmt::Display for VersionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VersionSource::Env => "DX_VERSION",
            VersionSource::ModuleBazel => "MODULE.bazel",
            VersionSource::Bazel => "bazel",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Env(String),
    Module {
        version: String,
        pins: Option<String>,
    },
    Bazel,
}

pub type EnvGet<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn utf8_env(env_get: EnvGet, name: &str) -> Result<Option<String>, LaunchError> {
    match env_get(name).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .ok()
            .map(Some)
            .ok_or_else(|| LaunchError::usage(format!("{name} must be valid UTF-8"))),
    }
}

pub fn resolve_version(env_get: EnvGet, cwd: &Path) -> Result<Route, LaunchError> {
    if let Some(value) = utf8_env(env_get, "DX_VERSION")? {
        if !version_is_valid(&value) {
            return Err(LaunchError::usage(format!(
                "DX_VERSION must be a plain semver such as 1.2.3, got '{value}'"
            )));
        }
        return Ok(Route::Env(value));
    }
    let Some(module) = find_module_file(cwd) else {
        return Ok(Route::Bazel);
    };
    let Some(version) = read_optional(&module)?.and_then(|text| module_version(&text)) else {
        return Ok(Route::Bazel);
    };
    let pins = match module.parent() {
        Some(root) => read_optional(&root.join(VERSION_FILE))?,
        None => None,
    };
    Ok(Route::Module { version, pins })
}

#[must_use]
pub fn find_module_file(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .map(|dir| dir.join("MODULE.bazel"))
        .find(|candidate| candidate.is_file())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
}

pub const SUPPORTED: &str = "linux x86_64/arm64, macos arm64, windows x86_64/arm64";
#[must_use]
pub fn platform(os: &str, arch: &str) -> Option<Platform> {
    let os = match os {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => return None,
    };
    if (os, arch) == ("macos", "x86_64") {
        return None;
    }
    Some(Platform { os, arch })
}

#[must_use]
pub fn unsupported_platform(version: &str, os: &str, arch: &str) -> LaunchError {
    LaunchError::failure(format!(
        "no dx {version} binary for {os} {arch}; supported: {SUPPORTED}; use bazel run @rules_dx//dx/launch:dx -- ..."
    ))
}
pub fn effective_arch(
    os: &'static str,
    arch: &'static str,
    sysctl: &dyn Fn(&str) -> Option<String>,
) -> &'static str {
    if os != "macos" || arch != "x86_64" {
        return arch;
    }
    let on_apple_silicon = ["hw.optional.arm64", "sysctl.proc_translated"]
        .iter()
        .any(|key| sysctl(key).is_some_and(|value| value.trim() == "1"));
    if on_apple_silicon { "aarch64" } else { arch }
}

const SYSCTL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn read_sysctl(key: &str) -> Option<String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<_> = std::env::split_paths(&path).collect();
    dirs.extend(["/usr/sbin".into(), "/sbin".into()]);
    let mut command = std::process::Command::new("sysctl");
    command
        .args(["-n", key])
        .env("PATH", std::env::join_paths(dirs).ok()?);
    let output = dx_process::capture::run_captured(&mut command, SYSCTL_TIMEOUT).ok()?;
    String::from_utf8(output.stdout).ok()
}

#[must_use]
pub fn host_platform() -> (&'static str, &'static str) {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    (os, effective_arch(os, arch, &read_sysctl))
}
#[must_use]
pub fn asset_name(platform: Platform) -> String {
    format!("dx-{}-{}", platform.os, platform.arch)
}
#[must_use]
pub fn cache_file_name(platform: Platform) -> String {
    let exe = if platform.os == "windows" { ".exe" } else { "" };
    format!("{}{exe}", asset_name(platform))
}
pub fn cache_root(env_get: EnvGet, windows: bool) -> Result<Option<PathBuf>, LaunchError> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    if let Some(dir) = non_empty("DX_CACHE_DIR") {
        let dir = PathBuf::from(dir);
        // has_root, not is_absolute: Windows calls a rooted `\cache` relative, but it does not follow the cwd.
        if !dir.has_root() {
            return Err(LaunchError::usage(format!(
                "DX_CACHE_DIR must be an absolute path, got '{}'",
                dir.display()
            )));
        }
        return Ok(Some(dir));
    }
    if let Some(xdg) = non_empty("XDG_CACHE_HOME").map(PathBuf::from)
        && xdg.has_root()
    {
        return Ok(Some(xdg.join("dx")));
    }
    let name = if windows { "USERPROFILE" } else { "HOME" };
    let Some(home) = non_empty(name).map(PathBuf::from) else {
        return Ok(None);
    };
    if !home.has_root() {
        return Err(LaunchError::usage(format!(
            "{name} must be an absolute path, got '{}'; set DX_CACHE_DIR",
            home.display()
        )));
    }
    Ok(Some(home.join(".cache").join("dx")))
}
#[must_use]
pub fn bin_dir(root: &Path, version: &str) -> PathBuf {
    root.join("bin").join(version)
}
#[must_use]
pub fn bin_path(root: &Path, version: &str, platform: Platform) -> PathBuf {
    bin_dir(root, version).join(cache_file_name(platform))
}
#[must_use]
pub fn release_url(base: &str, version: &str, asset: &str) -> String {
    format!("{}/v{version}/{asset}", base.trim_end_matches('/'))
}
pub fn release_base(env_get: EnvGet) -> Result<String, LaunchError> {
    let Some(base) = utf8_env(env_get, "DX_RELEASE_BASE_URL")? else {
        return Ok(DEFAULT_RELEASE_BASE.to_owned());
    };
    if base_is_allowed(&base) {
        Ok(base)
    } else {
        Err(LaunchError::usage(format!(
            "DX_RELEASE_BASE_URL must use https:// with a host and no credentials (http:// only to 127.0.0.1, [::1] or localhost), got '{}'",
            redact(&base)
        )))
    }
}

fn redact(base: &str) -> String {
    match base
        .split_once("://")
        .map(|(scheme, rest)| (scheme, rest.split_once('@')))
    {
        Some((scheme, Some((_, host)))) => format!("{scheme}://<redacted>@{host}"),
        _ => base.to_owned(),
    }
}

#[must_use]
pub fn base_is_allowed(base: &str) -> bool {
    if let Some(rest) = base.strip_prefix("https://") {
        let authority = rest.split('/').next().unwrap_or("");
        return !authority.contains('@') && !authority.starts_with(':') && !authority.is_empty();
    }
    let Some(rest) = base.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
