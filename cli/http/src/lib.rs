#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

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

pub const CA_BUNDLE_ENV: &str = "SSL_CERT_FILE";

const CHUNK: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub https_only: bool,
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub attempts: u32,
    pub first_backoff: Duration,
    pub max_backoff: Duration,
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

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Config(String),
    #[error("HTTP {code} from {url}")]
    Status { url: String, code: u16 },
    #[error("{url}: {message}")]
    Transport { url: String, message: String },
    #[error("{url}: the body is larger than {limit} bytes")]
    TooLarge { url: String, limit: u64 },
    #[error("cannot store {url}: {message}")]
    Write { url: String, message: String },
    #[error("{0}")]
    Blocked(String),
}

#[derive(Debug)]
pub struct Client {
    agent: ureq::Agent,
    proxies: Proxies,
    settings: Settings,
    rewrites: Rewrites,
}

impl Client {
    pub fn new(settings: Settings) -> Result<Self, Error> {
        Self::with_env(settings, |name| std::env::var(name).ok())
    }

    pub fn with_env(
        settings: Settings,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, Error> {
        let env = |name: &str| env(name).filter(|value| !value.trim().is_empty());
        let proxies = Proxies::from_env(&env);
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

    pub fn head(&self, url: &str, headers: &[(&str, &str)]) -> Result<HeaderMap, Error> {
        self.each_url(url, |url| {
            self.retrying(|| {
                self.call(Method::Head, url, headers)
                    .map(|response| response.headers().clone())
            })
        })
    }

    pub fn bytes(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<Vec<u8>, Error> {
        let mut body = Vec::new();
        self.stream(url, headers, limit, &mut body)?;
        Ok(body)
    }

    pub fn text(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<String, Error> {
        String::from_utf8(self.bytes(url, headers, limit)?).map_err(|error| Error::Transport {
            url: url.to_owned(),
            message: format!("the body is not UTF-8 text: {error}"),
        })
    }

    pub fn sha256(&self, url: &str, headers: &[(&str, &str)], limit: u64) -> Result<String, Error> {
        let mut sink = Sha256Writer::new();
        self.stream(url, headers, limit, &mut sink)?;
        Ok(sink.finish_hex())
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
}

#[derive(Debug)]
pub enum Body<'a> {
    Empty,
    Bytes(&'a [u8]),
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

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    #[must_use]
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }
}

fn retryable(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

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
    let transient = matches!(
        &error,
        ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::Protocol(_)
    ) || matches!(
        &error,
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
