use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use crate::protocol::ShimError;

pub const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_SIDECAR_BYTES: u64 = 4 * 1024;

const NEXT_STEP: &str = "; check network and proxy settings, see https://github.com/devhusdk/rules_dx/blob/main/docs/proxy-and-ca.md";

fn mismatch_message(url: &str, want: &str, got: &str) -> String {
    format!(
        "sha256 mismatch for {url}: want '{want}', got {got}; refusing to run it; retry, and if it repeats a proxy may be rewriting downloads{NEXT_STEP}"
    )
}

fn http_status_message(code: u16, url: &str) -> String {
    format!("download failed (HTTP {code}): {url}{NEXT_STEP}")
}

fn download_failed_message(url: &str, error: &dyn std::fmt::Display) -> String {
    format!("download failed: {url}: {error}{NEXT_STEP}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumSource {
    Sidecar,
    Pinned(String),
}

pub struct Request<'a> {
    pub version: &'a str,
    pub asset: &'a str,
    pub url: &'a str,
    pub dest: &'a Path,
    pub checksum: &'a ChecksumSource,
}

pub fn ensure(request: &Request<'_>) -> Result<PathBuf, ShimError> {
    if request.dest.is_file() {
        return Ok(request.dest.to_path_buf());
    }
    install(request)
}

pub fn install(request: &Request<'_>) -> Result<PathBuf, ShimError> {
    let dest = request.dest;
    let dir = dest
        .parent()
        .ok_or_else(|| ShimError::failure(format!("bad cache path {}", dest.display())))?;
    let not_writable = |e: &dyn std::fmt::Display| {
        ShimError::failure(format!(
            "cannot write the dx cache at {}: {e}; set DX_CACHE_DIR to a writable directory",
            dir.display()
        ))
    };
    std::fs::create_dir_all(dir).map_err(|e| not_writable(&e))?;
    let client = client(request.url)?;
    let want = match request.checksum {
        ChecksumSource::Sidecar => sidecar(&client, request)?,
        ChecksumSource::Pinned(hash) => parse_hash(hash).ok_or_else(|| {
            ShimError::failure(format!("bad pinned sha256 for {}: '{hash}'", request.url))
        })?,
    };
    sweep_stale_parts(dir, request.asset);
    let mut part = PartFile::create(dir, request.asset).map_err(|e| not_writable(&e))?;
    let got = download(&client, request, &mut part)?;
    if !dx_digest::verify(&want, &got) {
        return Err(ShimError::failure(mismatch_message(
            request.url,
            &want,
            &got,
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part.path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| not_writable(&e))?;
    }
    part.persist(dest).map_err(|e| not_writable(&e))?;
    Ok(dest.to_path_buf())
}

/// Release downloads may follow a plain `http://` base URL, such as a local mirror,
/// but an `https://` one never downgrades on a redirect.
fn client(url: &str) -> Result<dx_http::Client, ShimError> {
    dx_http::Client::new(dx_http::Settings {
        https_only: url.starts_with("https://"),
        user_agent: "rules_dx".to_owned(),
        ..dx_http::Settings::default()
    })
    .map_err(|e| ShimError::failure(format!("{e}{NEXT_STEP}")))
}

fn fetch_message(request: &Request<'_>, error: dx_http::Error) -> String {
    match error {
        dx_http::Error::Status { code: 404, url } => format!(
            "dx {} has no release asset {} (HTTP 404): {url}",
            request.version, request.asset
        ),
        dx_http::Error::Status { code, url } => http_status_message(code, &url),
        dx_http::Error::Transport { url, message } => download_failed_message(&url, &message),
        dx_http::Error::TooLarge { url, limit } => {
            download_failed_message(&url, &format!("larger than {limit} bytes"))
        }
        dx_http::Error::Write { message, .. } => message,
        dx_http::Error::Config(message) => format!("{message}{NEXT_STEP}"),
    }
}

fn download(
    client: &dx_http::Client,
    request: &Request<'_>,
    part: &mut PartFile,
) -> Result<String, ShimError> {
    let path = part.path.clone();
    let cannot_write = |e: &dyn std::fmt::Display| format!("cannot write {}: {e}", path.display());
    let written = match part.file.as_mut() {
        None => Err("the download part file is not open".to_owned()),
        Some(file) => client
            .download_into(request.url, &[], file, MAX_BINARY_BYTES)
            .map_err(|e| match e {
                dx_http::Error::Write { message, .. } => cannot_write(&message),
                other @ (dx_http::Error::Config(_)
                | dx_http::Error::Status { .. }
                | dx_http::Error::Transport { .. }
                | dx_http::Error::TooLarge { .. }) => fetch_message(request, other),
            })
            .and_then(|got| file.sync_all().map(|()| got).map_err(|e| cannot_write(&e))),
    };
    written.map_err(ShimError::failure)
}

fn sidecar(client: &dx_http::Client, request: &Request<'_>) -> Result<String, ShimError> {
    let url = format!("{}.sha256", request.url);
    let text = client
        .text(&url, &[], MAX_SIDECAR_BYTES)
        .map_err(|e| ShimError::failure(fetch_message(request, e)))?;
    let first = text.split_whitespace().next().unwrap_or("");
    parse_hash(first).ok_or_else(|| ShimError::failure(format!("{url} holds no sha256")))
}

#[must_use]
pub fn parse_hash(text: &str) -> Option<String> {
    dx_digest::is_hex_any_case(text, dx_digest::DIGEST_LEN).then(|| text.to_ascii_lowercase())
}

const STALE_PART_AGE: Duration = Duration::from_hours(1);

// A killed run leaves its part file behind.
fn sweep_stale_parts(dir: &Path, asset: &str) {
    let prefix = format!(".{asset}.");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(&prefix) && name.ends_with(".part")) {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok());
        if age.is_some_and(|age| age > STALE_PART_AGE) {
            drop(std::fs::remove_file(entry.path()));
        }
    }
}

#[cfg(unix)]
mod interrupt {
    use dx_os_sys::signals::{keep_on_stop, unlink_on_stop};
    use std::path::Path;

    pub fn watch(path: &Path) {
        if let Err(err) = unlink_on_stop(path) {
            eprintln!("dx: warning: cannot install the interrupt handler: {err}");
        }
    }

    pub fn clear() {
        keep_on_stop();
    }
}

#[cfg(windows)]
mod interrupt {
    use dx_os_sys::console::{CtrlEvent, CtrlHandler, add_handler};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, Once};

    static PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

    /// Removes the part file being downloaded and exits with the signal exit code.
    struct RemovePart;

    impl CtrlHandler for RemovePart {
        #[expect(
            clippy::exit,
            reason = "a console control handler ends the process with the signal exit code"
        )]
        fn handle(event: CtrlEvent) -> bool {
            if let Some(path) = PATH.lock().ok().and_then(|p| p.clone()) {
                let _ = std::fs::remove_file(path);
            }
            std::process::exit(match event {
                CtrlEvent::C => 130,
                CtrlEvent::Break | CtrlEvent::Close | CtrlEvent::Logoff | CtrlEvent::Shutdown => {
                    143
                }
            })
        }
    }

    pub fn watch(path: &Path) {
        static INSTALL: Once = Once::new();
        if let Ok(mut current) = PATH.lock() {
            *current = Some(path.to_path_buf());
        }
        INSTALL.call_once(|| {
            if let Err(err) = add_handler::<RemovePart>() {
                eprintln!("dx: warning: cannot install the Ctrl-C handler: {err}");
            }
        });
    }

    pub fn clear() {
        if let Ok(mut current) = PATH.lock() {
            *current = None;
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod interrupt {
    pub fn watch(_: &std::path::Path) {}
    pub fn clear() {}
}

struct PartFile {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl PartFile {
    fn create(dir: &Path, asset: &str) -> std::io::Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            ".{asset}.{}.{nanos}-{nonce}.part",
            std::process::id()
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        interrupt::watch(&path);
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn persist(mut self, dest: &Path) -> std::io::Result<()> {
        drop(self.file.take());
        interrupt::clear();
        match std::fs::rename(&self.path, dest) {
            Ok(()) => Ok(()),
            Err(_) if dest.is_file() => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        drop(self.file.take());
        interrupt::clear();
        drop(std::fs::remove_file(&self.path));
    }
}

#[cfg(test)]
mod tests {
    use super::{download_failed_message, http_status_message, mismatch_message};

    const DOC: &str = "see https://github.com/devhusdk/rules_dx/blob/main/docs/proxy-and-ca.md";

    #[test]
    fn mismatch_points_at_retry_and_proxies() {
        assert_eq!(
            mismatch_message("https://h/dx", "aa", "bb"),
            format!(
                "sha256 mismatch for https://h/dx: want 'aa', got bb; refusing to run it; retry, and if it repeats a proxy may be rewriting downloads; check network and proxy settings, {DOC}"
            )
        );
    }

    #[test]
    fn http_status_points_at_proxy_docs() {
        assert_eq!(
            http_status_message(503, "https://h/dx"),
            format!(
                "download failed (HTTP 503): https://h/dx; check network and proxy settings, {DOC}"
            )
        );
    }

    #[test]
    fn transport_failure_points_at_proxy_docs() {
        assert_eq!(
            download_failed_message("https://h/dx", &"reset"),
            format!(
                "download failed: https://h/dx: reset; check network and proxy settings, {DOC}"
            )
        );
    }
}
