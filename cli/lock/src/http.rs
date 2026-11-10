//! Archive downloads over HTTPS.

use std::time::Duration;

use crate::{Error, Fetch};

/// Downloads archives with the platform's certificate verifier.
pub struct HttpFetch(ureq::Agent);

fn agent(https_only: bool) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .https_only(https_only)
        .http_status_as_error(false)
        .tls_config(tls)
        .proxy(ureq::Proxy::try_from_env())
        .user_agent("rules_dx")
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .build()
        .into()
}

impl HttpFetch {
    /// A fetcher that refuses any URL that is not HTTPS.
    #[must_use]
    pub fn new() -> Self {
        Self(agent(true))
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
    /// Fails when the request fails or the status is not 200.
    pub fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        let mut response = self.0.get(url).call().map_err(|error| Error::new(error.to_string()))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(Error::new(format!("HTTP {status}")));
        }
        response
            .body_mut()
            .with_config()
            .limit(1 << 30)
            .read_to_vec()
            .map_err(|error| Error::new(error.to_string()))
    }
}

impl Fetch for HttpFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.bytes(url).map(|bytes| dx_digest::sha256_hex(&bytes))
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
            let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).expect("head");
            stream.write_all(body.as_bytes()).expect("body");
        });
        format!("http://127.0.0.1:{port}/archive")
    }

    #[test]
    fn a_download_returns_the_sha256_of_its_bytes() {
        let url = serve("200 OK", "alpha");
        assert_eq!(HttpFetch(agent(false)).sha256(&url), Ok(dx_digest::sha256_hex(b"alpha")));
    }

    #[test]
    fn a_download_returns_its_bytes() {
        let url = serve("200 OK", "alpha");
        assert_eq!(HttpFetch(agent(false)).bytes(&url), Ok(b"alpha".to_vec()));
    }

    #[test]
    fn a_missing_archive_names_the_status() {
        let url = serve("404 Not Found", "gone");
        assert_eq!(HttpFetch(agent(false)).sha256(&url), Err(Error::new("HTTP 404")));
    }

    #[test]
    fn a_plain_http_url_is_refused() {
        let error = HttpFetch::new().sha256("http://127.0.0.1:9/archive").expect_err("refused");
        assert!(error.to_string().to_lowercase().contains("https"), "{error}");
    }
}
