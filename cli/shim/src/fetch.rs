use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

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
    let agent = agent(request.url);
    let want = match request.checksum {
        ChecksumSource::Sidecar => sidecar(&agent, request)?,
        ChecksumSource::Pinned(hash) => parse_hash(hash).ok_or_else(|| {
            ShimError::failure(format!("bad pinned sha256 for {}: '{hash}'", request.url))
        })?,
    };
    sweep_stale_parts(dir, request.asset);
    let part = PartFile::create(dir, request.asset).map_err(|e| not_writable(&e))?;
    let got = download(&agent, request, &part)?;
    if got != want {
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

fn tls_config() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build()
}

fn agent(url: &str) -> ureq::Agent {
    let https = url.starts_with("https://");
    let proxy = if https {
        ureq::Proxy::try_from_env()
    } else {
        None
    };
    ureq::Agent::config_builder()
        .https_only(https)
        .tls_config(tls_config())
        .proxy(proxy)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        // ureq bounds the whole body transfer, not idle time. Keep it
        // generous so slow links can fetch a large binary (curl in tools/dx
        // has no limit); this only stops a transfer that has truly stalled.
        .timeout_recv_body(Some(Duration::from_secs(60 * 60)))
        .build()
        .into()
}

fn get(agent: &ureq::Agent, request: &Request<'_>, url: &str) -> Result<ureq::Body, ShimError> {
    match agent.get(url).call() {
        Ok(response) => Ok(response.into_body()),
        Err(ureq::Error::StatusCode(404)) => Err(ShimError::failure(format!(
            "dx {} has no release asset {} (HTTP 404): {url}",
            request.version, request.asset
        ))),
        Err(ureq::Error::StatusCode(code)) => {
            Err(ShimError::failure(http_status_message(code, url)))
        }
        Err(e) => Err(ShimError::failure(download_failed_message(url, &e))),
    }
}

fn download(
    agent: &ureq::Agent,
    request: &Request<'_>,
    part: &PartFile,
) -> Result<String, ShimError> {
    let body = get(agent, request, request.url)?;
    let mut reader = body.into_with_config().limit(MAX_BINARY_BYTES).reader();
    let mut file = part
        .file
        .as_ref()
        .ok_or_else(|| ShimError::failure("the download part file is not open"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let failed =
        |e: &dyn std::fmt::Display| ShimError::failure(download_failed_message(request.url, e));
    loop {
        let n = reader.read(&mut buf).map_err(|e| failed(&e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| {
            ShimError::failure(format!("cannot write {}: {e}", part.path.display()))
        })?;
    }
    file.sync_all()
        .map_err(|e| ShimError::failure(format!("cannot write {}: {e}", part.path.display())))?;
    Ok(hex(&hasher.finalize()))
}

fn sidecar(agent: &ureq::Agent, request: &Request<'_>) -> Result<String, ShimError> {
    let url = format!("{}.sha256", request.url);
    let body = get(agent, request, &url)?;
    let mut text = String::new();
    body.into_with_config()
        .limit(MAX_SIDECAR_BYTES)
        .reader()
        .read_to_string(&mut text)
        .map_err(|e| ShimError::failure(download_failed_message(&url, &e)))?;
    let first = text.split_whitespace().next().unwrap_or("");
    parse_hash(first).ok_or_else(|| ShimError::failure(format!("{url} holds no sha256")))
}

pub fn parse_hash(text: &str) -> Option<String> {
    (text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| text.to_ascii_lowercase())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const STALE_PART_AGE: Duration = Duration::from_secs(60 * 60);

// A killed run leaves its part file behind. Parts younger than an hour may
// belong to a download that is still running, so only older ones go.
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
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(unix)]
mod interrupt {
    use std::ffi::{CString, c_char, c_int};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::Once;
    use std::sync::atomic::{AtomicPtr, Ordering};

    const SIGINT: c_int = 2;
    const SIGTERM: c_int = 15;

    unsafe extern "C" {
        fn signal(signum: c_int, handler: usize) -> usize;
        fn unlink(path: *const c_char) -> c_int;
        fn _exit(code: c_int) -> !;
    }

    static PATH: AtomicPtr<c_char> = AtomicPtr::new(std::ptr::null_mut());

    extern "C" fn on_signal(signum: c_int) {
        let path = PATH.load(Ordering::SeqCst);
        if !path.is_null() {
            unsafe { unlink(path) };
        }
        unsafe { _exit(128 + signum) }
    }

    pub fn watch(path: &Path) {
        static INSTALL: Once = Once::new();
        let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
            return;
        };
        PATH.store(path.into_raw(), Ordering::SeqCst);
        INSTALL.call_once(|| unsafe {
            signal(SIGINT, on_signal as *const () as usize);
            signal(SIGTERM, on_signal as *const () as usize);
        });
    }

    pub fn clear() {
        PATH.store(std::ptr::null_mut(), Ordering::SeqCst);
    }
}

#[cfg(windows)]
mod interrupt {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, Once};

    static PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

    unsafe extern "system" fn on_ctrl(ctrl_type: u32) -> i32 {
        if let Some(path) = PATH.lock().ok().and_then(|p| p.clone()) {
            let _ = std::fs::remove_file(path);
        }
        std::process::exit(128 + if ctrl_type == 0 { 2 } else { 15 })
    }

    pub fn watch(path: &Path) {
        static INSTALL: Once = Once::new();
        if let Ok(mut current) = PATH.lock() {
            *current = Some(path.to_path_buf());
        }
        INSTALL.call_once(|| unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(on_ctrl), 1);
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
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
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
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{download_failed_message, http_status_message, mismatch_message, tls_config};

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

    #[test]
    fn tls_uses_the_os_trust_store() {
        assert!(matches!(
            tls_config().root_certs(),
            ureq::tls::RootCerts::PlatformVerifier
        ));
    }
}
