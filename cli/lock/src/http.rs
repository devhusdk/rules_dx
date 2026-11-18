//! Archive downloads over HTTPS.

use dx_http::{Client, Rewrites, Settings};

use crate::{Error, Fetch};

/// The largest registry document or archive a lock step reads.
const BODY_LIMIT: u64 = 1 << 30;

/// Downloads registry documents and archives through the shared dx HTTP client.
pub struct HttpFetch(Result<Client, Error>);

fn settings() -> Settings {
    Settings {
        https_only: true,
        user_agent: "rules_dx".to_owned(),
        ..Settings::default()
    }
}

/// The variable that names the downloader config Bazel uses, as an absolute path.
pub const DOWNLOADER_CONFIG_ENV: &str = "DX_DOWNLOADER_CONFIG";

fn with_config(
    client: Result<Client, dx_http::Error>,
    config: Option<&std::path::Path>,
) -> Result<Client, Error> {
    let client = client.map_err(lock_error)?;
    let Some(path) = config else {
        return Ok(client);
    };
    let rewrites = Rewrites::load(path).map_err(lock_error)?;
    Ok(client.with_rewrites(rewrites))
}

fn lock_error(error: dx_http::Error) -> Error {
    match error {
        dx_http::Error::Status { code, .. } => Error::new(format!("HTTP {code}")),
        other @ (dx_http::Error::Config(_)
        | dx_http::Error::Blocked(_)
        | dx_http::Error::Transport { .. }
        | dx_http::Error::TooLarge { .. }
        | dx_http::Error::Write { .. }) => Error::new(other.to_string()),
    }
}

impl HttpFetch {
    /// A fetcher that refuses any URL that is not HTTPS. A bad proxy or CA setting fails each download.
    ///
    /// When `DX_DOWNLOADER_CONFIG` names a Bazel downloader config, every request goes through
    /// its rewrites, allow list and block list. A config that cannot be read fails each download.
    #[must_use]
    pub fn new() -> Self {
        let config = std::env::var_os(DOWNLOADER_CONFIG_ENV).filter(|value| !value.is_empty());
        Self(with_config(
            Client::new(settings()),
            config.as_deref().map(std::path::Path::new),
        ))
    }

    fn client(&self) -> Result<&Client, Error> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

impl Default for HttpFetch {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFetch {
    /// Downloads `url` and returns its bytes.
    ///
    /// # Errors
    /// Fails when the request fails or the status is not 2xx.
    pub fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.accepting(url, "*/*")
    }

    /// Downloads `url` with the `Accept` header `accept` and returns its bytes.
    ///
    /// # Errors
    /// Fails when the request fails or the status is not 2xx.
    pub fn accepting(&self, url: &str, accept: &str) -> Result<Vec<u8>, Error> {
        self.client()?
            .bytes(url, &[("Accept", accept)], BODY_LIMIT)
            .map_err(lock_error)
    }
}

impl Fetch for HttpFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.client()?
            .sha256(url, &[], BODY_LIMIT)
            .map_err(lock_error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use super::*;

    /// Serves one connection per response, in order, on a loopback port and returns its URL.
    fn serve(responses: &[(&'static str, &'static str)]) -> String {
        let responses = responses.to_vec();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("address").port();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).expect("read") == 1
                {
                    request.push(byte[0]);
                }
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).expect("head");
                stream.write_all(body.as_bytes()).expect("body");
            }
        });
        format!("http://127.0.0.1:{port}/archive")
    }

    /// A fetcher for loopback URLs that ignores the host's proxy settings.
    fn plain() -> HttpFetch {
        let settings = Settings {
            https_only: false,
            first_backoff: std::time::Duration::from_millis(10),
            ..settings()
        };
        HttpFetch(Client::with_env(settings, |_| None).map_err(lock_error))
    }

    #[test]
    fn a_download_returns_the_sha256_of_its_bytes() {
        let url = serve(&[("200 OK", "alpha")]);
        assert_eq!(plain().sha256(&url), Ok(dx_digest::sha256_hex(b"alpha")));
    }

    #[test]
    fn a_download_returns_its_bytes() {
        let url = serve(&[("200 OK", "alpha")]);
        assert_eq!(plain().bytes(&url), Ok(b"alpha".to_vec()));
    }

    #[test]
    fn a_flaky_server_is_retried() {
        let url = serve(&[("503 Service Unavailable", "busy"), ("200 OK", "alpha")]);
        assert_eq!(plain().sha256(&url), Ok(dx_digest::sha256_hex(b"alpha")));
    }

    #[test]
    fn a_missing_archive_names_the_status() {
        let url = serve(&[("404 Not Found", "gone")]);
        assert_eq!(plain().sha256(&url), Err(Error::new("HTTP 404")));
    }

    /// A loopback fetcher whose requests go through the downloader config `text`.
    fn configured(text: &str) -> HttpFetch {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("downloader.cfg");
        std::fs::write(&path, text).expect("config");
        let settings = Settings {
            https_only: false,
            first_backoff: std::time::Duration::from_millis(10),
            ..settings()
        };
        HttpFetch(with_config(
            Client::with_env(settings, |_| None),
            Some(&path),
        ))
    }

    #[test]
    fn a_blocked_upstream_is_fetched_from_its_mirror() {
        let mirror = serve(&[("200 OK", "alpha")]);
        let base = mirror.strip_suffix("/archive").expect("mirror base");
        let fetch = configured(&format!(
            "block upstream.invalid\nrewrite upstream\\.invalid/(.*) {base}/$1\n"
        ));
        assert_eq!(
            fetch.sha256("https://upstream.invalid/archive"),
            Ok(dx_digest::sha256_hex(b"alpha"))
        );
    }

    #[test]
    fn a_config_that_blocks_everything_names_its_message() {
        let fetch = configured("block *\nall_blocked_message Use the corporate mirror.\n");
        let error = fetch
            .sha256("https://upstream.invalid/archive")
            .expect_err("blocked");
        assert!(
            error.to_string().ends_with(" - Use the corporate mirror."),
            "{error}"
        );
    }

    #[test]
    fn an_unreadable_config_fails_each_download() {
        let fetch = HttpFetch(with_config(
            Client::with_env(settings(), |_| None),
            Some(std::path::Path::new("/nonexistent/downloader.cfg")),
        ));
        let error = fetch
            .bytes("https://upstream.invalid/a")
            .expect_err("config");
        assert!(error.to_string().contains("downloader.cfg"), "{error}");
    }

    #[test]
    fn a_plain_http_url_is_refused() {
        let error = HttpFetch::new()
            .sha256("http://127.0.0.1:9/archive")
            .expect_err("refused");
        assert!(
            error.to_string().to_lowercase().contains("https"),
            "{error}"
        );
    }
}
