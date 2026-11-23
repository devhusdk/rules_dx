use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use dx_error::stdio::write_line;

use crate::fetch::{self, ChecksumSource};
use crate::protocol::{self, Platform, Route, ShimError, VersionSource};

pub struct Resolved {
    pub version: String,
    pub source: VersionSource,
    pub platform: Platform,
    pub bin: PathBuf,
    pub checksum: Option<ChecksumSource>,
}

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
    let root = protocol::cache_root(env_get, windows)?.ok_or_else(|| {
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
    if fetch::cached(&resolved.bin, resolved.checksum.as_ref()) {
        return Ok(resolved.bin.clone());
    }
    let Some(checksum) = &resolved.checksum else {
        return Err(ShimError::failure("no pinned checksum for the download"));
    };
    let base = protocol::release_base(env_get)?;
    let asset = protocol::asset_name(resolved.platform);
    let url = protocol::release_url(&base, &resolved.version, &asset);
    fetch::install(&fetch::Request {
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

#[must_use]
pub fn main(args: Vec<OsString>) -> i32 {
    match run(args) {
        Ok(code) => code,
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("{error}"));
            error.code
        }
    }
}

fn run(args: Vec<OsString>) -> Result<i32, ShimError> {
    if let Some((first, rest)) = args.split_first()
        && first == "shim"
    {
        return shim_command(rest);
    }
    let cwd = std::env::current_dir()
        .map_err(|e| ShimError::failure(format!("cannot read the current directory: {e}")))?;
    let Some(resolved) = resolve(&env_get, &cwd)? else {
        return via_bazel(args);
    };
    let bin = fetch(&env_get, &resolved)?;
    exec(&bin, args)
}

fn emit(out: &mut dyn Write, text: std::fmt::Arguments<'_>) -> Result<(), ShimError> {
    write_line(out, text).map_err(|e| ShimError::failure(format!("cannot write to stdout: {e}")))
}

fn emit_path(out: &mut dyn Write, label: &str, path: &Path) -> Result<(), ShimError> {
    let mut bytes = label.as_bytes().to_vec();
    bytes.extend_from_slice(path.as_os_str().as_encoded_bytes());
    bytes.push(b'\n');
    match out.write_all(&bytes) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            Err(ShimError::failure(format!("cannot write to stdout: {e}")))
        }
        Ok(()) | Err(_) => Ok(()),
    }
}

fn shim_command(args: &[OsString]) -> Result<i32, ShimError> {
    shim_command_to(args, &mut std::io::stdout().lock())
}

fn shim_command_to(args: &[OsString], out: &mut dyn Write) -> Result<i32, ShimError> {
    let sub = args.first().map(|arg| arg.to_string_lossy().into_owned());
    match sub.as_deref() {
        Some("version") if args.len() == 1 => {
            emit(out, format_args!("dx-shim {}", protocol::SHIM_VERSION))?;
            Ok(0)
        }
        Some("where") if args.len() == 1 => {
            let cwd = std::env::current_dir().map_err(|e| {
                ShimError::failure(format!("cannot read the current directory: {e}"))
            })?;
            let Some(resolved) = resolve(&env_get, &cwd)? else {
                emit(out, format_args!("source: {}", VersionSource::Bazel))?;
                return Ok(0);
            };
            emit(out, format_args!("version: {}", resolved.version))?;
            emit(out, format_args!("source: {}", resolved.source))?;
            emit_path(out, "binary: ", &resolved.bin)?;
            emit(
                out,
                format_args!(
                    "cached: {}",
                    if resolved.bin.is_file() { "yes" } else { "no" }
                ),
            )?;
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

const DX: &str = "@rules_dx//:dx";

fn via_bazel(args: Vec<OsString>) -> Result<i32, ShimError> {
    let mut full: Vec<OsString> = vec!["run".into(), DX.into(), "--".into()];
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
    use dx_os_sys::console::{CtrlEvent, CtrlHandler, add_handler};

    struct Swallow;

    impl CtrlHandler for Swallow {
        fn handle(_event: CtrlEvent) -> bool {
            true
        }
    }

    if let Err(err) = add_handler::<Swallow>() {
        dx_error::stdio::eprint_line(format_args!(
            "dx: warning: cannot install the Ctrl-C handler: {err}"
        ));
    }
    let status = std::process::Command::new(bin)
        .args(args)
        .env("DX_SHIM", "1")
        .status()
        .map_err(|e| ShimError::failure(format!("cannot run {}: {e}", bin.display())))?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn bazel_runs_the_dx_command_target() {
        assert_eq!(DX, dx_families::DX);
    }

    #[test]
    fn version_prints_the_shim_version() {
        let mut out = Vec::new();
        let code = shim_command_to(&["version".into()], &mut out).expect("version");
        assert_eq!(code, 0);
        assert_eq!(
            String::from_utf8(out).expect("utf-8"),
            format!("dx-shim {}\n", protocol::SHIM_VERSION)
        );
    }

    #[test]
    fn version_into_a_closed_pipe_succeeds() {
        let (reader, mut writer) = std::io::pipe().expect("pipe");
        drop(reader);
        let code = shim_command_to(&["version".into()], &mut writer).expect("closed pipe");
        assert_eq!(code, 0);
    }

    #[test]
    fn version_into_an_open_pipe_reaches_the_reader() {
        let (mut reader, mut writer) = std::io::pipe().expect("pipe");
        shim_command_to(&["version".into()], &mut writer).expect("version");
        drop(writer);
        let mut text = String::new();
        reader.read_to_string(&mut text).expect("read");
        assert_eq!(text, format!("dx-shim {}\n", protocol::SHIM_VERSION));
    }
}
