use std::path::{Path, PathBuf};

pub const SHIM_VERSION: &str = "0.0.0";
pub const DEFAULT_RELEASE_BASE: &str = "https://github.com/devhusdk/rules_dx/releases/download";

pub const VERSION_FILE: &str = ".dx/version";
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
pub fn parse_pin(text: &str) -> Option<String> {
    let first = text.split('\n').next().unwrap_or("");
    let pin: String = first.chars().filter(|c| *c != '\r' && *c != ' ').collect();
    version_is_valid(&pin).then_some(pin)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSource {
    Env,
    File(PathBuf),
}

impl std::fmt::Display for VersionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionSource::Env => f.write_str("DX_VERSION"),
            VersionSource::File(path) => write!(f, "{}", path.display()),
        }
    }
}
pub fn resolve_version(
    env_get: &dyn Fn(&str) -> Option<String>,
    cwd: &Path,
) -> Result<(String, VersionSource), ShimError> {
    if let Some(value) = env_get("DX_VERSION").filter(|value| !value.is_empty()) {
        if !version_is_valid(&value) {
            return Err(ShimError::usage(format!(
                "DX_VERSION must be a plain semver such as 1.2.3, got '{value}'"
            )));
        }
        return Ok((value, VersionSource::Env));
    }
    let Some(file) = find_version_file(cwd) else {
        return Err(ShimError::failure(format!(
            "not in a dx workspace (no .dx/version above {}); run 'dx init' from a Bazel workspace with bazel run @rules_dx//:dx -- init --apply, or set DX_VERSION",
            cwd.display()
        )));
    };
    let text = std::fs::read_to_string(&file)
        .map_err(|e| ShimError::failure(format!("cannot read {}: {e}", file.display())))?;
    match parse_pin(&text) {
        Some(pin) => Ok((pin, VersionSource::File(file))),
        None => Err(ShimError::failure(format!(
            "{} does not hold a plain semver such as 1.2.3; fix it with bazel run @rules_dx//:dx -- init --apply",
            file.display()
        ))),
    }
}

pub fn find_version_file(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .map(|dir| dir.join(VERSION_FILE))
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
pub fn host_platform() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}
pub fn asset_name(platform: Platform) -> String {
    format!("dx-{}-{}", platform.os, platform.arch)
}
pub fn cache_file_name(platform: Platform) -> String {
    let exe = if platform.os == "windows" { ".exe" } else { "" };
    format!("{}{exe}", asset_name(platform))
}
pub fn cache_root(env_get: &dyn Fn(&str) -> Option<String>, windows: bool) -> Option<PathBuf> {
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
pub fn release_base(env_get: &dyn Fn(&str) -> Option<String>) -> Result<String, ShimError> {
    let Some(base) = env_get("DX_RELEASE_BASE_URL").filter(|value| !value.is_empty()) else {
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
