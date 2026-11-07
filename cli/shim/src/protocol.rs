use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const SHIM_VERSION: &str = "0.0.0";
pub const DEFAULT_RELEASE_BASE: &str = "https://github.com/devhusdk/rules_dx/releases/download";

pub const VERSION_FILE: &str = ".dx/pins";
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_FAILURE: i32 = 1;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimError {
    pub code: i32,
    pub message: String,
}

impl ShimError {
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

impl std::fmt::Display for ShimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dx: {}", self.message)
    }
}

impl std::error::Error for ShimError {}
pub fn version_is_valid(version: &str) -> bool {
    !version.is_empty() && semver::Version::parse(version).is_ok()
}
/// The sha256 on the `<version> <sha256> <asset>` line of a `.dx/pins` file.
pub fn pinned_hash(text: &str, version: &str, asset: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let fields: Vec<&str> = line.trim_end_matches('\r').split_whitespace().collect();
        let [line_version, hash, line_asset] = fields[..] else {
            return None;
        };
        let is_hash = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        (line_version == version && line_asset == asset && is_hash).then(|| hash.to_owned())
    })
}

/// The version of the single root `bazel_dep(name = "rules_dx", ...)` line, by the
/// same rules as the `tools/dx` launchers. `None` sends the call through Bazel.
pub fn module_version(text: &str) -> Option<String> {
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
    /// `DX_VERSION` bootstrap: the checksum comes from the release sidecar.
    Env(String),
    /// Version from MODULE.bazel; `pins` is the raw `.dx/pins` file.
    Module {
        version: String,
        pins: Option<String>,
    },
    Bazel,
}

pub type EnvGet<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn utf8_env(env_get: EnvGet, name: &str) -> Result<Option<String>, ShimError> {
    match env_get(name).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .map(Some)
            .map_err(|_| ShimError::usage(format!("{name} must be valid UTF-8"))),
    }
}

pub fn resolve_version(env_get: EnvGet, cwd: &Path) -> Result<Route, ShimError> {
    if let Some(value) = utf8_env(env_get, "DX_VERSION")? {
        if !version_is_valid(&value) {
            return Err(ShimError::usage(format!(
                "DX_VERSION must be a plain semver such as 1.2.3, got '{value}'"
            )));
        }
        return Ok(Route::Env(value));
    }
    let Some(module) = find_module_file(cwd) else {
        return Ok(Route::Bazel);
    };
    let Some(version) = std::fs::read_to_string(&module)
        .ok()
        .and_then(|text| module_version(&text))
    else {
        return Ok(Route::Bazel);
    };
    let pins = module
        .parent()
        .map(|root| root.join(VERSION_FILE))
        .and_then(|file| std::fs::read_to_string(file).ok());
    Ok(Route::Module { version, pins })
}

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

pub fn unsupported_platform(version: &str, os: &str, arch: &str) -> ShimError {
    ShimError::failure(format!(
        "no dx {version} binary for {os} {arch}; supported: {SUPPORTED}; use bazel run @rules_dx//:dx -- ..."
    ))
}
/// An x86_64 process on an Apple silicon Mac (Rosetta) runs on arm64 hardware.
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

fn read_sysctl(key: &str) -> Option<String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<_> = std::env::split_paths(&path).collect();
    dirs.extend(["/usr/sbin".into(), "/sbin".into()]);
    let output = std::process::Command::new("sysctl")
        .args(["-n", key])
        .env("PATH", std::env::join_paths(dirs).ok()?)
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()
}

pub fn host_platform() -> (&'static str, &'static str) {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    (os, effective_arch(os, arch, &read_sysctl))
}
pub fn asset_name(platform: Platform) -> String {
    format!("dx-{}-{}", platform.os, platform.arch)
}
pub fn cache_file_name(platform: Platform) -> String {
    let exe = if platform.os == "windows" { ".exe" } else { "" };
    format!("{}{exe}", asset_name(platform))
}
pub fn cache_root(env_get: EnvGet, windows: bool) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    if let Some(dir) = non_empty("DX_CACHE_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Some(xdg) = non_empty("XDG_CACHE_HOME") {
        return Some(PathBuf::from(xdg).join("dx"));
    }
    let home = if windows { "USERPROFILE" } else { "HOME" };
    non_empty(home).map(|home| PathBuf::from(home).join(".cache").join("dx"))
}
pub fn bin_dir(root: &Path, version: &str) -> PathBuf {
    root.join("bin").join(version)
}
pub fn bin_path(root: &Path, version: &str, platform: Platform) -> PathBuf {
    bin_dir(root, version).join(cache_file_name(platform))
}
pub fn release_url(base: &str, version: &str, asset: &str) -> String {
    format!("{}/v{version}/{asset}", base.trim_end_matches('/'))
}
pub fn release_base(env_get: EnvGet) -> Result<String, ShimError> {
    let Some(base) = utf8_env(env_get, "DX_RELEASE_BASE_URL")? else {
        return Ok(DEFAULT_RELEASE_BASE.to_owned());
    };
    if base_is_allowed(&base) {
        Ok(base)
    } else {
        Err(ShimError::usage(format!(
            "DX_RELEASE_BASE_URL must use https:// (http:// only to 127.0.0.1, [::1] or localhost), got '{base}'"
        )))
    }
}

pub fn base_is_allowed(base: &str) -> bool {
    if base.starts_with("https://") && base.len() > "https://".len() {
        return true;
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
