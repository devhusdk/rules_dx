use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::fetch::{self, ChecksumSource};
use crate::protocol::{self, Platform, ShimError, VersionSource};

pub struct Resolved {
    pub version: String,
    pub source: VersionSource,
    pub platform: Platform,
    pub bin: PathBuf,
}

pub fn resolve(
    env_get: &dyn Fn(&str) -> Option<String>,
    cwd: &Path,
) -> Result<Resolved, ShimError> {
    let (version, source) = protocol::resolve_version(env_get, cwd)?;
    let (os, arch) = protocol::host_platform();
    let platform = protocol::platform(os, arch)
        .ok_or_else(|| protocol::unsupported_platform(&version, os, arch))?;
    let windows = platform.os == "windows";
    let root = protocol::cache_root(env_get, windows).ok_or_else(|| {
        ShimError::failure(if windows {
            "USERPROFILE is not set; set DX_CACHE_DIR"
        } else {
            "HOME is not set; set DX_CACHE_DIR"
        })
    })?;
    let bin = protocol::bin_path(&root, &version, platform);
    Ok(Resolved {
        version,
        source,
        platform,
        bin,
    })
}

pub fn fetch(
    env_get: &dyn Fn(&str) -> Option<String>,
    resolved: &Resolved,
) -> Result<PathBuf, ShimError> {
    if resolved.bin.is_file() {
        return Ok(resolved.bin.clone());
    }
    let base = protocol::release_base(env_get)?;
    let asset = protocol::asset_name(resolved.platform);
    let url = protocol::release_url(&base, &resolved.version, &asset);
    fetch::ensure(&fetch::Request {
        version: &resolved.version,
        asset: &asset,
        url: &url,
        dest: &resolved.bin,
        checksum: &ChecksumSource::Sidecar,
    })
}

fn env_get(name: &str) -> Option<String> {
    std::env::var(name).ok()
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
    let resolved = resolve(&env_get, &cwd)?;
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
            let resolved = resolve(&env_get, &cwd)?;
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
    // real handler is not inherited: the child keeps default handling and
    // the shim waits for its exit code (same approach as cargo and rustup).
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
