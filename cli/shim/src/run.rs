use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::fetch::{self, ChecksumSource};
use crate::protocol::{self, Platform, Route, ShimError, VersionSource};

pub struct Resolved {
    pub version: String,
    pub source: VersionSource,
    pub platform: Platform,
    pub bin: PathBuf,
    pub checksum: Option<ChecksumSource>,
}

/// `None` means the call goes through `bazel run @rules_dx//:dx`.
pub fn resolve(env_get: protocol::EnvGet, cwd: &Path) -> Result<Option<Resolved>, ShimError> {
    resolve_on(env_get, cwd, protocol::host_platform())
}

pub fn resolve_on(
    env_get: protocol::EnvGet,
    cwd: &Path,
    (os, arch): (&str, &str),
) -> Result<Option<Resolved>, ShimError> {
    let (version, source, pins) = match protocol::resolve_version(env_get, cwd)? {
        Route::Bazel => return Ok(None),
        Route::Env(version) => (version, VersionSource::Env, None),
        Route::Module { version, pins } => (version, VersionSource::ModuleBazel, Some(pins)),
    };
    let Some(platform) = protocol::platform(os, arch) else {
        if pins.is_some() {
            return Ok(None);
        }
        return Err(protocol::unsupported_platform(&version, os, arch));
    };
    let windows = platform.os == "windows";
    let root = protocol::cache_root(env_get, windows).ok_or_else(|| {
        ShimError::failure(if windows {
            "USERPROFILE is not set; set DX_CACHE_DIR"
        } else {
            "HOME is not set; set DX_CACHE_DIR"
        })
    })?;
    let bin = protocol::bin_path(&root, &version, platform);
    let checksum = match pins {
        None => Some(ChecksumSource::Sidecar),
        Some(pins) => {
            let asset = protocol::asset_name(platform);
            let hash = pins.and_then(|text| protocol::pinned_hash(&text, &version, &asset));
            match hash {
                Some(hash) => Some(ChecksumSource::Pinned(hash)),
                None if bin.is_file() => None,
                None => return Ok(None),
            }
        }
    };
    Ok(Some(Resolved {
        version,
        source,
        platform,
        bin,
        checksum,
    }))
}

pub fn fetch(env_get: protocol::EnvGet, resolved: &Resolved) -> Result<PathBuf, ShimError> {
    if resolved.bin.is_file() {
        return Ok(resolved.bin.clone());
    }
    let Some(checksum) = &resolved.checksum else {
        return Err(ShimError::failure("no pinned checksum for the download"));
    };
    let base = protocol::release_base(env_get)?;
    let asset = protocol::asset_name(resolved.platform);
    let url = protocol::release_url(&base, &resolved.version, &asset);
    fetch::ensure(&fetch::Request {
        version: &resolved.version,
        asset: &asset,
        url: &url,
        dest: &resolved.bin,
        checksum,
    })
}

fn env_get(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

pub fn main(args: Vec<OsString>) -> i32 {
    match run(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            error.code
        }
    }
}

fn run(args: Vec<OsString>) -> Result<i32, ShimError> {
    if args.first().is_some_and(|first| first == "shim") {
        return shim_command(&args[1..]);
    }
    let cwd = std::env::current_dir()
        .map_err(|e| ShimError::failure(format!("cannot read the current directory: {e}")))?;
    let Some(resolved) = resolve(&env_get, &cwd)? else {
        return via_bazel(args);
    };
    let bin = fetch(&env_get, &resolved)?;
    exec(&bin, args)
}

fn shim_command(args: &[OsString]) -> Result<i32, ShimError> {
    let sub = args.first().map(|arg| arg.to_string_lossy().into_owned());
    match sub.as_deref() {
        Some("version") if args.len() == 1 => {
            println!("dx-shim {}", protocol::SHIM_VERSION);
            Ok(0)
        }
        Some("where") if args.len() == 1 => {
            let cwd = std::env::current_dir().map_err(|e| {
                ShimError::failure(format!("cannot read the current directory: {e}"))
            })?;
            let Some(resolved) = resolve(&env_get, &cwd)? else {
                println!("source: {}", VersionSource::Bazel);
                return Ok(0);
            };
            println!("version: {}", resolved.version);
            println!("source: {}", resolved.source);
            println!("binary: {}", resolved.bin.display());
            println!(
                "cached: {}",
                if resolved.bin.is_file() { "yes" } else { "no" }
            );
            Ok(0)
        }
        Some(sub @ ("version" | "where")) => Err(ShimError::usage(format!(
            "dx shim {sub} takes no arguments"
        ))),
        Some(sub @ ("self-update" | "prune")) => Err(ShimError::usage(format!(
            "dx shim {sub} is not available yet"
        ))),
        Some(sub) => Err(ShimError::usage(format!(
            "unknown dx shim command '{sub}'; use dx shim version or dx shim where"
        ))),
        None => Err(ShimError::usage(
            "dx shim needs a command: dx shim version or dx shim where",
        )),
    }
}

fn via_bazel(args: Vec<OsString>) -> Result<i32, ShimError> {
    let mut full: Vec<OsString> = vec!["run".into(), "@rules_dx//:dx".into(), "--".into()];
    full.extend(args);
    exec(Path::new("bazel"), full)
}

#[cfg(unix)]
fn exec(bin: &Path, args: Vec<OsString>) -> Result<i32, ShimError> {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(bin)
        .args(args)
        .env("DX_SHIM", "1")
        .exec();
    Err(ShimError::failure(format!(
        "cannot run {}: {error}",
        bin.display()
    )))
}

#[cfg(windows)]
fn exec(bin: &Path, args: Vec<OsString>) -> Result<i32, ShimError> {
    // Register a handler that swallows Ctrl+C/Ctrl+Break for the shim
    // only. Passing `None` would set the process-wide "ignore Ctrl+C" flag,
    // which child processes inherit, so dx.exe would ignore Ctrl+C too. A
    // real handler is not inherited: dx.exe installs its own handler and the
    // shim waits for its exit code (same approach as cargo and rustup).
    unsafe extern "system" fn swallow_ctrl(_ctrl_type: u32) -> i32 {
        1
    }
    // SAFETY: registers a static function with no captured state.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(swallow_ctrl), 1);
    }
    let status = std::process::Command::new(bin)
        .args(args)
        .env("DX_SHIM", "1")
        .status()
        .map_err(|e| ShimError::failure(format!("cannot run {}: {e}", bin.display())))?;
    Ok(status.code().unwrap_or(1))
}
