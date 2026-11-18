//! Places the prebuilt dx binary the `tools/dx` launcher runs into the dx cache, so the
//! launcher starts without a download once `dx fetch` has run.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dx_output::{NoticeEvent, OutputMode, notice_event};
use dx_shim::protocol;

use super::common::emit_built;
use super::notice_codes::{CODE_DX_CACHED, CODE_DX_NOT_CACHED};

/// Where the launcher's binary stands after `dx fetch`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Cached {
    /// No `rules_dx` version or no pin for this host, so the launcher runs dx through Bazel.
    NotPinned {
        asset: String,
    },
    /// The host has no prebuilt dx, so the launcher runs dx through Bazel.
    Unsupported {
        os: String,
        arch: String,
    },
    /// No cache directory can be named because HOME or USERPROFILE is unset.
    NoCacheDir {
        home: &'static str,
    },
    Present {
        version: String,
        path: PathBuf,
    },
    Placed {
        version: String,
        path: PathBuf,
    },
    /// The running dx is not the pinned binary, so it was not copied.
    Mismatch {
        version: String,
        asset: String,
    },
}

/// What [`ensure`] reads from the host.
pub(crate) struct Host<'a> {
    pub workspace: &'a Path,
    pub env_get: protocol::EnvGet<'a>,
    pub os: &'a str,
    pub arch: &'a str,
    /// The running dx binary.
    pub exe: &'a Path,
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn annotate(path: &Path, error: &io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// Makes sure the launcher's cache holds the pinned dx for the host, copying the running
/// binary only when its sha256 equals the pin.
pub(crate) fn ensure(host: &Host<'_>) -> Result<Cached, String> {
    let module = host.workspace.join("MODULE.bazel");
    let version = read_optional(&module)
        .map_err(|error| annotate(&module, &error))?
        .and_then(|text| protocol::module_version(&text));
    let Some(platform) = protocol::platform(host.os, host.arch) else {
        return Ok(Cached::Unsupported {
            os: host.os.to_owned(),
            arch: host.arch.to_owned(),
        });
    };
    let asset = protocol::asset_name(platform);
    let Some(version) = version else {
        return Ok(Cached::NotPinned { asset });
    };
    let windows = platform.os == "windows";
    let Some(root) = protocol::cache_root(host.env_get, windows) else {
        return Ok(Cached::NoCacheDir {
            home: if windows { "USERPROFILE" } else { "HOME" },
        });
    };
    let path = protocol::bin_path(&root, &version, platform);
    if path.is_file() {
        return Ok(Cached::Present { version, path });
    }
    let pins_file = host.workspace.join(protocol::VERSION_FILE);
    let pin = read_optional(&pins_file)
        .map_err(|error| annotate(&pins_file, &error))?
        .and_then(|text| protocol::pinned_hash(&text, &version, &asset));
    let Some(pin) = pin else {
        return Ok(Cached::NotPinned { asset });
    };
    let running =
        dx_digest::sha256_file_hex(host.exe).map_err(|error| annotate(host.exe, &error))?;
    if !running.eq_ignore_ascii_case(&pin) {
        return Ok(Cached::Mismatch { version, asset });
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| annotate(dir, &error))?;
    }
    dx_atomic_fs::copy_atomic(host.exe, &path).map_err(|error| annotate(&path, &error))?;
    // The running binary can be replaced while it is copied; never leave a file the
    // launcher would trust without checking.
    let copied = dx_digest::sha256_file_hex(&path).map_err(|error| annotate(&path, &error))?;
    if !copied.eq_ignore_ascii_case(&pin) {
        std::fs::remove_file(&path).ok();
        return Err(format!(
            "{}: the copy does not match the pin in {}",
            path.display(),
            protocol::VERSION_FILE
        ));
    }
    Ok(Cached::Placed { version, path })
}

/// The notice for `cached`: info when the launcher is ready, warning when it is not.
pub(crate) fn notice(cached: &Cached) -> NoticeEvent {
    match cached {
        Cached::NotPinned { asset } => NoticeEvent::info(
            CODE_DX_NOT_CACHED,
            format!(
                "no prebuilt dx ({asset}) is pinned in MODULE.bazel and {}; tools/dx runs dx through Bazel",
                protocol::VERSION_FILE
            ),
        ),
        Cached::Unsupported { os, arch } => NoticeEvent::info(
            CODE_DX_NOT_CACHED,
            format!("no prebuilt dx for {os} {arch}; tools/dx runs dx through Bazel"),
        ),
        Cached::NoCacheDir { home } => NoticeEvent::warning(
            CODE_DX_NOT_CACHED,
            format!("did not cache dx: {home} is not set; set DX_CACHE_DIR"),
        ),
        Cached::Present { version, path } => NoticeEvent::info(
            CODE_DX_CACHED,
            format!("dx {version} is already cached at {}", path.display()),
        ),
        Cached::Placed { version, path } => NoticeEvent::info(
            CODE_DX_CACHED,
            format!("cached dx {version} at {}", path.display()),
        ),
        Cached::Mismatch { version, asset } => NoticeEvent::warning(
            CODE_DX_NOT_CACHED,
            format!(
                "did not cache dx {version}: the running dx is not the {asset} pinned in {}; next: run tools/dx while online so it downloads the pinned dx",
                protocol::VERSION_FILE
            ),
        ),
    }
}

/// Runs [`ensure`] for this process and reports the outcome on stderr, and as a JSON notice.
/// `Ok(false)` means the cache could not be written; the error is already reported.
pub(crate) fn cache_running_dx(
    workspace: &Path,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let (os, arch) = protocol::host_platform();
    let cached = std::env::current_exe()
        .map_err(|error| format!("locating the running dx: {error}"))
        .and_then(|exe| {
            ensure(&Host {
                workspace,
                env_get: &|name| std::env::var_os(name),
                os,
                arch,
                exe: &exe,
            })
        });
    report(&cached, output, out, err)
}

fn report(
    cached: &Result<Cached, String>,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, i32> {
    let notice = match cached {
        Ok(cached) => notice(cached),
        Err(message) => {
            NoticeEvent::warning(CODE_DX_NOT_CACHED, format!("could not cache dx: {message}"))
        }
    };
    if notice.severity == "info" {
        writeln!(err, "dx: {}", notice.message).ok();
    } else {
        writeln!(err, "dx: warning: {}: {}", notice.code, notice.message).ok();
    }
    if output == OutputMode::Json {
        emit_built(out, notice_event(&notice))?;
    }
    Ok(cached.is_ok())
}

#[cfg(test)]
#[path = "dx_asset_tests.rs"]
mod tests;
