use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::protocol::ShimError;

pub const MAX_BINARY_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_SIDECAR_BYTES: u64 = 4 * 1024;

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
    let part = PartFile::create(dir, request.asset).map_err(|e| not_writable(&e))?;
    let agent = agent(request.url);
    let got = download(&agent, request, &part)?;
    let want = match request.checksum {
        ChecksumSource::Sidecar => sidecar(&agent, request)?,
        ChecksumSource::Pinned(hash) => parse_hash(hash).ok_or_else(|| {
            ShimError::failure(format!("bad pinned sha256 for {}: '{hash}'", request.url))
        })?,
    };
    if got != want {
        return Err(ShimError::failure(format!(
            "sha256 mismatch for {}: want '{want}', got {got}; refusing to run it",
            request.url
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

fn agent(url: &str) -> ureq::Agent {
    let https = url.starts_with("https://");
    let proxy = if https {
        ureq::Proxy::try_from_env()
    } else {
        None
    };
    ureq::Agent::config_builder()
        .https_only(https)
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
        Err(ureq::Error::StatusCode(code)) => Err(ShimError::failure(format!(
            "download failed (HTTP {code}): {url}"
        ))),
        Err(e) => Err(ShimError::failure(format!("download failed: {url}: {e}"))),
    }
}

fn download(
    agent: &ureq::Agent,
    request: &Request<'_>,
    part: &PartFile,
) -> Result<String, ShimError> {
    let body = get(agent, request, request.url)?;
    let mut reader = body.into_with_config().limit(MAX_BINARY_BYTES).reader();
    let mut file = part.file.as_ref().expect("open part file");
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let failed = |e: &dyn std::fmt::Display| {
        ShimError::failure(format!("download failed: {}: {e}", request.url))
    };
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
        .map_err(|e| ShimError::failure(format!("download failed: {url}: {e}")))?;
    let first = text.split_whitespace().next().unwrap_or("");
    parse_hash(first).ok_or_else(|| {
        ShimError::failure(format!(
            "sha256 mismatch for {}: want '{first}', the sidecar holds no sha256; refusing to run it",
            request.url
        ))
    })
}

pub fn parse_hash(text: &str) -> Option<String> {
    (text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| text.to_ascii_lowercase())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
        Ok(Self {
            path,
            file: Some(file),
        })
    }

    fn persist(mut self, dest: &Path) -> std::io::Result<()> {
        drop(self.file.take());
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
        let _ = std::fs::remove_file(&self.path);
    }
}
