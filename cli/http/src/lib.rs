#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

//! The one HTTP client dx uses: proxy and CA trust from the environment, explicit
//! timeouts, retries with backoff, and downloads that hash while they stream.

mod proxy;
mod rewrites;

#[cfg(test)]
mod lib_tests;

use std::fs::File;
use std::io::{self, Read as _, Seek as _};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use dx_digest::Sha256Writer;
use ureq::RequestBuilder;
use ureq::tls::{PemItem, RootCerts, TlsConfig, TlsProvider};
use ureq::typestate::WithBody;

use proxy::Proxies;

pub use rewrites::Rewrites;

pub use ureq::http::HeaderMap;

/// The variable that names a PEM bundle to trust instead of the OS trust store.
pub const CA_BUNDLE_ENV: &str = "SSL_CERT_FILE";

const CHUNK: usize = 64 * 1024;

/// Timeouts, retries and the identity of one client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Refuse `http://` URLs and redirects to them.
    pub https_only: bool,
    /// The `User-Agent` header.
    pub user_agent: String,
    /// Longest wait to resolve a host and open a connection to it.
    pub connect_timeout: Duration,
    /// Longest wait for any single read or write. A stalled server fails after this;
    /// a transfer that keeps moving has no overall deadline.
    pub idle_timeout: Duration,
    /// Tries per request, counting the first. Zero counts as one.
    pub attempts: u32,
    /// Wait before the first retry. It doubles for each later retry.
    pub first_backoff: Duration,
    /// Longest wait between two tries.
    pub max_backoff: Duration,
    /// Follow 3xx answers. When false, [`Client::send`] returns the 3xx response itself.
    pub follow_redirects: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            https_only: true,
            user_agent: "rules_dx".to_owned(),
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(60),
            attempts: 3,
            first_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(8),
            follow_redirects: true,
        }
    }
}

/// Why a request failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// A proxy variable or the CA bundle is unusable.
    #[error("{0}")]
    Config(String),
    /// The server answered with a status outside 2xx.
    #[error("HTTP {code} from {url}")]
    Status { url: String, code: u16 },
    /// The request never got a complete answer.
    #[error("{url}: {message}")]
    Transport { url: String, message: String },
    /// The body is larger than the caller allows.
    #[error("{url}: the body is larger than {limit} bytes")]
    TooLarge { url: String, limit: u64 },
    /// The body arrived but could not be stored.
    #[error("cannot store {url}: {message}")]
    Write { url: String, message: String },
    /// The downloader config blocks every URL the request could use.
    #[error("{0}")]
    Blocked(String),
}

/// One configured HTTP client. Cheap to share by reference across threads.
#[derive(Debug)]
pub struct Client {
    agent: ureq::Agent,
    proxies: Proxies,
    settings: Settings,
    rewrites: Rewrites,
}

impl Client {
    /// A client that reads proxy and CA settings from the process environment.
    ///
    /// # Errors
    /// Fails when the CA bundle cannot be read. A proxy variable that is not a proxy URL
    /// fails each request that would go through it instead.
    pub fn new(settings: Settings) -> Result<Self, Error> {
        Self::with_env(settings, |name| std::env::var(name).ok())
    }

    /// A client that reads proxy and CA settings through `env` instead of the process environment.
    ///
    /// # Errors
    /// Fails when the CA bundle cannot be read. A proxy variable that is not a proxy URL
    /// fails each request that would go through it instead.
    pub fn with_env(
        settings: Settings,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, Error> {
        // An empty variable means unset, as it does for curl.
        let env = |name: &str| env(name).filter(|value| !value.trim().is_empty());
        let proxies = Proxies::from_env(&env);
        // The provider is set here rather than left to ureq's feature flags, so the
        // TLS stack does not depend on which other crates enable which features.
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .root_certs(root_certs(&env)?)
            .build();
        let agent = ureq::Agent::config_builder()
            .https_only(settings.https_only)
            .http_status_as_error(false)
            .tls_config(tls)
            .user_agent(settings.user_agent.as_str())
            .timeout_resolve(Some(settings.connect_timeout))
            .timeout_connect(Some(settings.connect_timeout))
            .timeout_per_read(Some(settings.idle_timeout))
            .timeout_per_write(Some(settings.idle_timeout))
            .max_redirects(if settings.follow_redirects { 10 } else { 0 })
            .build()
            .into();
        Ok(Self {
            agent,
            proxies,
            settings,
            rewrites: Rewrites::default(),
        })
    }

    /// This client with downloads and `HEAD` requests sent through `rewrites`. Each one tries
    /// the rewritten URLs in order and returns the first success, else the last failure.
    #[must_use]
    pub fn with_rewrites(self, rewrites: Rewrites) -> Self {
        Self { rewrites, ..self }
    }

    fn each_url<T>(
        &self,
        url: &str,
        mut attempt: impl FnMut(&str) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut last = None;
        for candidate in self.rewrites.amend(url)? {
            match attempt(&candidate) {
                Ok(value) => return Ok(value),
                Err(error) => last = Some(error),
            }
        }
        Err(last.unwrap_or_else(|| Error::Blocked(url.to_owned())))
    }

    /// Sends `HEAD url` and returns the response headers.
    ///
    /// # Errors
    /// Fails on a status outside 2xx or when every try fails.
    pub fn head(&self, url: &str, headers: &[(&str, &str)]) -> Result<HeaderMap, Error> {
        self.each_url(url, |url| {
            self.retrying(|| {
                self.call(Method::Head, url, headers)
                    .map(|response| response.headers().clone())
            })
        })
    }

    /// Downloads `url` into memory, refusing a body over `limit` bytes.
    ///
    /// # Errors
    /// Fails on a status outside 2xx, an oversized body, or when every try fails.
    pub fn bytes(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<Vec<u8>, Error> {
        let mut body = Vec::new();
        self.stream(url, headers, limit, &mut body)?;
        Ok(body)
    }

    /// Downloads `url` as UTF-8 text, refusing a body over `limit` bytes.
    ///
    /// # Errors
    /// Fails like [`Client::bytes`], and when the body is not UTF-8.
    pub fn text(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<String, Error> {
        String::from_utf8(self.bytes(url, headers, limit)?).map_err(|_| Error::Transport {
            url: url.to_owned(),
            message: "the body is not UTF-8 text".to_owned(),
        })
    }

    /// Streams `url` through sha256 without keeping it and returns the hex digest.
    ///
    /// # Errors
    /// Fails like [`Client::bytes`].
    pub fn sha256(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<String, Error> {
        let mut sink = Sha256Writer::new();
        self.stream(url, headers, limit, &mut sink)?;
        Ok(sink.finish_hex())
    }

    /// Streams `url` into `file`, hashing as it writes, and returns the hex sha256.
    /// Each retry truncates `file` and starts over, so it holds exactly one copy of the body.
    ///
    /// # Errors
    /// Fails like [`Client::bytes`], and when `file` cannot be written.
    pub fn download_into(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        file: &mut File,
        limit: u64,
    ) -> Result<String, Error> {
        let mut sink = FileSink {
            file,
            hash: Sha256Writer::new(),
        };
        self.stream(url, headers, limit, &mut sink)?;
        Ok(sink.hash.finish_hex())
    }

    /// Streams `url` into a staging file beside `dest` and renames it over `dest` once
    /// the whole body has arrived. Returns the hex sha256. On failure `dest` is untouched
    /// and no staging file is left behind.
    ///
    /// # Errors
    /// Fails like [`Client::download_into`], and when `dest` cannot be staged or replaced.
    pub fn download_to(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        dest: &Path,
        limit: u64,
    ) -> Result<String, Error> {
        let store_failed = |error: io::Error| Error::Write {
            url: url.to_owned(),
            message: format!("{}: {error}", dest.display()),
        };
        dx_atomic_fs::remove_stale_staging(dest);
        let mut staged = dx_atomic_fs::StagedFile::create(dest).map_err(store_failed)?;
        let sha = self.download_into(url, headers, staged.file_mut(), limit)?;
        staged.commit().map_err(store_failed)?;
        Ok(sha)
    }

    /// Sends one request and returns the answer whatever its status, keeping at most the
    /// first `keep` bytes of the response body.
    ///
    /// GET and HEAD send no body and are retried on transport errors and on 408, 429 and
    /// 5xx answers; when the tries run out the last answer is returned. POST and PUT are
    /// sent once, so a publish never repeats.
    ///
    /// # Errors
    /// Fails when no answer arrives, or when a proxy variable is unusable.
    pub fn send(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Body<'_>,
        keep: u64,
    ) -> Result<Response, Error> {
        if matches!(method, Method::Post | Method::Put) {
            return self
                .exchange(method, url, headers, body, keep)
                .map_err(Failure::into_error);
        }
        let mut tries = 1;
        let mut backoff = self.settings.first_backoff;
        loop {
            let last = tries >= self.settings.attempts;
            match self.exchange(method, url, headers, Body::Empty, keep) {
                Ok(response) if last || !retryable(response.status) => return Ok(response),
                Err(Failure::Final(error)) => return Err(error),
                Err(Failure::Transient(error)) if last => return Err(error),
                Ok(_) | Err(Failure::Transient(_)) => {}
            }
            std::thread::sleep(backoff);
            backoff = backoff.saturating_mul(2).min(self.settings.max_backoff);
            tries += 1;
        }
    }

    fn exchange(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Body<'_>,
        keep: u64,
    ) -> Result<Response, Failure> {
        let response = self.request(method, url, headers, body)?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let mut body = Vec::new();
        response
            .into_body()
            .into_reader()
            .take(keep)
            .read_to_end(&mut body)
            .map_err(|error| classify(url, ureq::Error::from(error)))?;
        Ok(Response {
            status,
            headers,
            body,
        })
    }

    fn stream(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        limit: u64,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        self.each_url(url, |url| self.stream_one(url, headers, limit, sink))
    }

    fn stream_one(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        limit: u64,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let store_failed = |error: &io::Error| {
            Failure::Final(Error::Write {
                url: url.to_owned(),
                message: error.to_string(),
            })
        };
        self.retrying(|| {
            sink.restart().map_err(|error| store_failed(&error))?;
            let response = self.call(Method::Get, url, headers)?;
            let mut reader = response
                .into_body()
                .into_with_config()
                .limit(limit)
                .reader();
            let mut buf = vec![0_u8; CHUNK];
            loop {
                let read = match reader.read(&mut buf) {
                    Ok(0) => return Ok(()),
                    Ok(read) => read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(classify(url, ureq::Error::from(error))),
                };
                let Some(chunk) = buf.get(..read) else {
                    return Err(store_failed(&io::ErrorKind::InvalidData.into()));
                };
                sink.write_all(chunk)
                    .map_err(|error| store_failed(&error))?;
            }
        })
    }

    fn request(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Body<'_>,
    ) -> Result<ureq::http::Response<ureq::Body>, Failure> {
        let proxy = self.proxies.for_url(url).map_err(Failure::Final)?;
        match method {
            Method::Get => prepare(self.agent.get(url), headers, proxy).call(),
            Method::Head => prepare(self.agent.head(url), headers, proxy).call(),
            Method::Post => body.send(prepare(self.agent.post(url), headers, proxy)),
            Method::Put => body.send(prepare(self.agent.put(url), headers, proxy)),
        }
        .map_err(|error| classify(url, error))
    }

    fn call(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<ureq::http::Response<ureq::Body>, Failure> {
        let response = self.request(method, url, headers, Body::Empty)?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let error = Error::Status {
            url: url.to_owned(),
            code: status.as_u16(),
        };
        // Only answers that say "try again" are retried; any other 4xx stays a 4xx.
        Err(if retryable(status.as_u16()) {
            Failure::Transient(error)
        } else {
            Failure::Final(error)
        })
    }

    fn retrying<T>(&self, mut attempt: impl FnMut() -> Result<T, Failure>) -> Result<T, Error> {
        let mut tries = 1;
        let mut backoff = self.settings.first_backoff;
        loop {
            match attempt() {
                Ok(value) => return Ok(value),
                Err(Failure::Final(error)) => return Err(error),
                Err(Failure::Transient(error)) if tries >= self.settings.attempts => {
                    return Err(error);
                }
                Err(Failure::Transient(_)) => {
                    std::thread::sleep(backoff);
                    backoff = backoff.saturating_mul(2).min(self.settings.max_backoff);
                    tries += 1;
                }
            }
        }
    }
}

/// The HTTP methods [`Client::send`] speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
}

/// What a POST or PUT carries.
#[derive(Debug)]
pub enum Body<'a> {
    Empty,
    Bytes(&'a [u8]),
    /// Streamed from the file's current position; set `Content-Length` to avoid chunking.
    File(File),
}

impl Body<'_> {
    fn send(
        self,
        request: RequestBuilder<WithBody>,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        match self {
            Body::Empty => request.send_empty(),
            Body::Bytes(bytes) => request.send(bytes),
            Body::File(file) => request.send(file),
        }
    }
}

/// One answer from [`Client::send`], whatever its status.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    /// The body, cut to the `keep` bytes the caller asked for.
    pub body: Vec<u8>,
}

impl Response {
    /// The first value of header `name`, when it is valid text.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// The body as text, with invalid UTF-8 replaced.
    #[must_use]
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }
}

/// Whether an answer says "try again"; any other 4xx stays a 4xx.
fn retryable(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

/// One try's failure: a transient one is worth another try, a final one is not.
enum Failure {
    Transient(Error),
    Final(Error),
}

impl Failure {
    fn into_error(self) -> Error {
        match self {
            Failure::Transient(error) | Failure::Final(error) => error,
        }
    }
}

fn prepare<B>(
    mut request: RequestBuilder<B>,
    headers: &[(&str, &str)],
    proxy: Option<ureq::Proxy>,
) -> RequestBuilder<B> {
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.config().proxy(proxy).build()
}

fn classify(url: &str, error: ureq::Error) -> Failure {
    if let ureq::Error::BodyExceedsLimit(limit) = error {
        return Failure::Final(Error::TooLarge {
            url: url.to_owned(),
            limit,
        });
    }
    // ureq::Error is non_exhaustive and some variants exist only under some cargo
    // features, so the transient ones are picked out and every other one is final.
    let transient = matches!(
        &error,
        ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::Protocol(_)
    ) || matches!(
        &error,
        // A certificate the TLS stack rejected stays rejected on the next try.
        ureq::Error::Io(io) if !io.get_ref().is_some_and(|inner| inner.is::<rustls::Error>())
    );
    let error = Error::Transport {
        url: url.to_owned(),
        message: error.to_string(),
    };
    if transient {
        Failure::Transient(error)
    } else {
        Failure::Final(error)
    }
}

fn root_certs(env: &dyn Fn(&str) -> Option<String>) -> Result<RootCerts, Error> {
    let Some(path) = env(CA_BUNDLE_ENV) else {
        return Ok(RootCerts::PlatformVerifier);
    };
    let unusable =
        |reason: &dyn std::fmt::Display| Error::Config(format!("{CA_BUNDLE_ENV}={path}: {reason}"));
    let pem = std::fs::read(&path).map_err(|error| unusable(&error))?;
    let mut certs = Vec::new();
    for item in ureq::tls::parse_pem(&pem) {
        if let PemItem::Certificate(cert) = item.map_err(|error| unusable(&error))? {
            certs.push(cert);
        }
    }
    if certs.is_empty() {
        return Err(unusable(&"the file holds no PEM certificate"));
    }
    Ok(RootCerts::from(certs))
}

/// Where a streamed body goes; `restart` drops what an earlier try wrote.
trait Sink: io::Write {
    fn restart(&mut self) -> io::Result<()>;
}

impl Sink for Vec<u8> {
    fn restart(&mut self) -> io::Result<()> {
        self.clear();
        Ok(())
    }
}

impl Sink for Sha256Writer {
    fn restart(&mut self) -> io::Result<()> {
        *self = Self::new();
        Ok(())
    }
}

struct FileSink<'a> {
    file: &'a mut File,
    hash: Sha256Writer,
}

impl io::Write for FileSink<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buf)?;
        let Some(chunk) = buf.get(..written) else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        self.hash.write_all(chunk)?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Sink for FileSink<'_> {
    fn restart(&mut self) -> io::Result<()> {
        self.file.set_len(0)?;
        self.file.rewind()?;
        self.hash = Sha256Writer::new();
        Ok(())
    }
}
