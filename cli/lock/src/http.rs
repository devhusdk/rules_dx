//! Archive downloads over HTTPS.

use dx_http::{Client, Settings};

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

fn lock_error(error: dx_http::Error) -> Error {
    match error {
        dx_http::Error::Status { code, .. } => Error::new(format!("HTTP {code}")),
        other @ (dx_http::Error::Config(_)
        | dx_http::Error::Transport { .. }
        | dx_http::Error::TooLarge { .. }
        | dx_http::Error::Write { .. }) => Error::new(other.to_string()),
    }
}

impl HttpFetch {
    /// A fetcher that refuses any URL that is not HTTPS. A bad proxy or CA setting fails each download.
    #[must_use]
    pub fn new() -> Self {
        Self(Client::new(settings()).map_err(lock_error))
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

    /// Serves one response on a loopback port and returns its URL.
    fn serve(status: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("address").port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).expect("read") == 1 {
                request.push(byte[0]);
            }
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).expect("head");
            stream.write_all(body.as_bytes()).expect("body");
        });
        format!("http://127.0.0.1:{port}/archive")
    }

    /// A fetcher for loopback URLs that ignores the host's proxy settings.
    fn plain() -> HttpFetch {
        let settings = Settings {
            https_only: false,
            ..settings()
        };
        HttpFetch(Client::with_env(settings, |_| None).map_err(lock_error))
    }

    #[test]
    fn a_download_returns_the_sha256_of_its_bytes() {
        let url = serve("200 OK", "alpha");
        assert_eq!(plain().sha256(&url), Ok(dx_digest::sha256_hex(b"alpha")));
    }

    #[test]
    fn a_download_returns_its_bytes() {
        let url = serve("200 OK", "alpha");
        assert_eq!(plain().bytes(&url), Ok(b"alpha".to_vec()));
    }

    #[test]
    fn a_missing_archive_names_the_status() {
        let url = serve("404 Not Found", "gone");
        assert_eq!(plain().sha256(&url), Err(Error::new("HTTP 404")));
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
