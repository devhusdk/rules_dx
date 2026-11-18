use std::io::{BufRead as _, BufReader, Read as _, Seek as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;

/// One canned answer per accepted connection; the last one repeats.
#[derive(Clone)]
enum Reply {
    Full(u16, &'static [u8]),
    /// Promises `length` bytes, sends `body`, then closes.
    Cut(usize, &'static [u8]),
    /// Promises `length` bytes, sends `body`, then goes silent with the socket open.
    Stall(usize, &'static [u8]),
}

/// A loopback server that records the first line of every request it gets.
struct Server {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn start(replies: &[Reply]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let replies = replies.to_vec();
        std::thread::spawn(move || {
            for (index, stream) in listener.incoming().enumerate() {
                let Ok(stream) = stream else { return };
                let reply = replies
                    .get(index)
                    .or(replies.last())
                    .cloned()
                    .expect("a reply");
                let log = Arc::clone(&log);
                std::thread::spawn(move || answer(stream, &reply, &log));
            }
        });
        Self { url, seen }
    }

    fn requests(&self) -> Vec<String> {
        self.seen.lock().expect("log").clone()
    }
}

fn answer(stream: TcpStream, reply: &Reply, log: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    log.lock().expect("log").push(first.trim_end().to_owned());
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line == "\r\n" => break,
            Ok(_) => {}
        }
    }
    let mut out = stream;
    let (code, length, body) = match reply {
        Reply::Full(code, body) => (*code, body.len(), *body),
        Reply::Cut(length, body) | Reply::Stall(length, body) => (200, *length, *body),
    };
    let head =
        format!("HTTP/1.1 {code} X\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
    drop(out.write_all(head.as_bytes()));
    drop(out.write_all(body));
    drop(out.flush());
    if let Reply::Stall(..) = reply {
        std::thread::sleep(Duration::from_secs(30));
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn quick() -> Settings {
    Settings {
        https_only: false,
        idle_timeout: Duration::from_secs(5),
        attempts: 3,
        first_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(20),
        ..Settings::default()
    }
}

fn client(settings: Settings) -> Client {
    Client::with_env(settings, no_env).expect("client")
}

#[test]
fn a_body_downloads_in_full() {
    let server = Server::start(&[Reply::Full(200, b"alpha")]);
    let url = format!("{}/a", server.url);
    assert_eq!(
        client(quick()).bytes(&url, &[], 1024),
        Ok(b"alpha".to_vec())
    );
    assert_eq!(
        client(quick()).text(&url, &[], 1024),
        Ok("alpha".to_owned())
    );
    assert_eq!(
        client(quick()).sha256(&url, &[], 1024),
        Ok(dx_digest::sha256_hex(b"alpha"))
    );
}

#[test]
fn a_busy_server_is_retried_until_it_answers() {
    let server = Server::start(&[
        Reply::Full(503, b""),
        Reply::Full(429, b""),
        Reply::Full(200, b"alpha"),
    ]);
    let url = format!("{}/a", server.url);
    assert_eq!(
        client(quick()).bytes(&url, &[], 1024),
        Ok(b"alpha".to_vec())
    );
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn retries_stop_after_the_last_attempt() {
    let server = Server::start(&[Reply::Full(503, b"")]);
    let url = format!("{}/a", server.url);
    assert_eq!(
        client(quick()).bytes(&url, &[], 1024),
        Err(Error::Status { url, code: 503 })
    );
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn a_missing_file_is_not_retried() {
    let server = Server::start(&[Reply::Full(404, b"")]);
    let url = format!("{}/a", server.url);
    assert_eq!(
        client(quick()).bytes(&url, &[], 1024),
        Err(Error::Status { url, code: 404 })
    );
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn a_cut_download_starts_over_and_the_file_holds_one_copy() {
    let server = Server::start(&[Reply::Cut(5, b"al"), Reply::Full(200, b"alpha")]);
    let url = format!("{}/a", server.url);
    let mut file = tempfile::tempfile().expect("file");
    assert_eq!(
        client(quick()).download_into(&url, &[], &mut file, 1024),
        Ok(dx_digest::sha256_hex(b"alpha"))
    );
    file.rewind().expect("rewind");
    let mut written = Vec::new();
    file.read_to_end(&mut written).expect("read");
    assert_eq!(written, b"alpha");
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_stalled_server_fails_after_the_idle_timeout() {
    let server = Server::start(&[Reply::Stall(1_000_000, b"partial")]);
    let url = format!("{}/a", server.url);
    let settings = Settings {
        idle_timeout: Duration::from_millis(200),
        attempts: 2,
        ..quick()
    };
    let started = Instant::now();
    let error = client(settings)
        .sha256(&url, &[], 10_000_000)
        .expect_err("stalled");
    assert!(
        matches!(&error, Error::Transport { url: failed, .. } if *failed == url),
        "{error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(server.requests().len(), 2, "the stall is retried");
}

#[test]
fn a_body_over_the_limit_is_refused_without_a_retry() {
    let server = Server::start(&[Reply::Full(200, b"0123456789")]);
    let url = format!("{}/a", server.url);
    assert_eq!(
        client(quick()).bytes(&url, &[], 4),
        Err(Error::TooLarge { url, limit: 4 })
    );
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn a_refused_connection_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/a", listener.local_addr().expect("address"));
    drop(listener);
    let error = client(quick()).bytes(&url, &[], 1024).expect_err("refused");
    assert!(matches!(&error, Error::Transport { .. }), "{error}");
}

#[test]
fn head_returns_the_headers() {
    let server = Server::start(&[Reply::Full(200, b"")]);
    let url = format!("{}/a", server.url);
    let headers = client(quick())
        .head(&url, &[("Accept", "x/y")])
        .expect("head");
    assert_eq!(
        headers.get("content-length").and_then(|v| v.to_str().ok()),
        Some("0")
    );
    assert_eq!(server.requests(), ["HEAD /a HTTP/1.1"]);
}

#[test]
fn https_only_refuses_plain_http() {
    let error = client(Settings::default())
        .bytes("http://127.0.0.1:9/a", &[], 1024)
        .expect_err("refused");
    assert!(error.to_string().contains("https"), "{error}");
}

/// An address nothing listens on, so a direct connection is refused at once.
fn closed_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address").to_string();
    drop(listener);
    address
}

/// Settings for a proxy test: one try, so the proxy sees one request per call.
fn once() -> Settings {
    Settings {
        attempts: 1,
        ..quick()
    }
}

#[test]
fn an_https_url_tunnels_through_https_proxy() {
    let proxy = Server::start(&[Reply::Full(502, b"")]);
    let proxy_url = proxy.url.clone();
    let env = move |name: &str| (name == "HTTPS_PROXY").then(|| proxy_url.clone());
    let client = Client::with_env(once(), env).expect("client");
    client
        .bytes("https://mirror.example/a", &[], 1024)
        .expect_err("the proxy refuses the tunnel");
    assert_eq!(proxy.requests(), ["CONNECT mirror.example:443 HTTP/1.1"]);
}

#[test]
fn http_proxy_alone_still_carries_https_urls() {
    let proxy = Server::start(&[Reply::Full(502, b"")]);
    let proxy_url = proxy.url.clone();
    let env = move |name: &str| (name == "http_proxy").then(|| proxy_url.clone());
    let client = Client::with_env(once(), env).expect("client");
    client
        .bytes("https://mirror.example/a", &[], 1024)
        .expect_err("the proxy refuses the tunnel");
    assert_eq!(proxy.requests(), ["CONNECT mirror.example:443 HTTP/1.1"]);
}

#[test]
fn a_plain_http_url_goes_direct() {
    let proxy = Server::start(&[Reply::Full(200, b"via proxy")]);
    let direct = Server::start(&[Reply::Full(200, b"direct")]);
    let proxy_url = proxy.url.clone();
    let env =
        move |name: &str| matches!(name, "http_proxy" | "HTTPS_PROXY").then(|| proxy_url.clone());
    let client = Client::with_env(once(), env).expect("client");
    assert_eq!(
        client.bytes(&format!("{}/a", direct.url), &[], 1024),
        Ok(b"direct".to_vec())
    );
    assert_eq!(proxy.requests(), [] as [String; 0]);
}

#[test]
fn no_proxy_hosts_skip_the_proxy() {
    let proxy = Server::start(&[Reply::Full(502, b"")]);
    let url = format!("https://{}/a", closed_address());
    let proxy_url = proxy.url.clone();
    let env = move |name: &str| match name {
        "HTTPS_PROXY" => Some(proxy_url.clone()),
        "NO_PROXY" => Some("localhost,127.0.0.1".to_owned()),
        _ => None,
    };
    let error = Client::with_env(once(), env)
        .expect("client")
        .bytes(&url, &[], 1024)
        .expect_err("nothing listens");
    assert!(matches!(&error, Error::Transport { .. }), "{error}");
    assert_eq!(proxy.requests(), [] as [String; 0]);
}

#[test]
fn an_empty_proxy_variable_counts_as_unset() {
    let proxy = Server::start(&[Reply::Full(502, b"")]);
    let proxy_url = proxy.url.clone();
    let env = move |name: &str| match name {
        "HTTPS_PROXY" => Some(" ".to_owned()),
        "http_proxy" => Some(proxy_url.clone()),
        _ => None,
    };
    let client = Client::with_env(once(), env).expect("client");
    client
        .bytes("https://mirror.example/a", &[], 1024)
        .expect_err("the proxy refuses the tunnel");
    assert_eq!(proxy.requests(), ["CONNECT mirror.example:443 HTTP/1.1"]);
}

#[test]
fn a_bad_proxy_variable_fails_only_the_requests_that_use_it() {
    let direct = Server::start(&[Reply::Full(200, b"direct")]);
    let env = |name: &str| (name == "https_proxy").then(|| "gopher://proxy".to_owned());
    let client = Client::with_env(once(), env).expect("client");
    assert_eq!(
        client.bytes("https://mirror.example/a", &[], 1024),
        Err(Error::Config("https_proxy is not a proxy URL".into()))
    );
    assert_eq!(
        client.bytes(&format!("{}/a", direct.url), &[], 1024),
        Ok(b"direct".to_vec())
    );
}

const CA: &str = include_str!("../testdata/ca.pem");

fn bundle_env(path: &std::path::Path) -> impl Fn(&str) -> Option<String> {
    let path = path.display().to_string();
    move |name| (name == CA_BUNDLE_ENV).then(|| path.clone())
}

#[test]
fn the_ca_bundle_replaces_the_os_trust_store() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("ca.pem");
    std::fs::write(&path, CA).expect("write");
    assert!(matches!(
        root_certs(&bundle_env(&path)),
        Ok(RootCerts::Specific(certs)) if certs.len() == 1
    ));
    assert!(matches!(
        root_certs(&no_env),
        Ok(RootCerts::PlatformVerifier)
    ));
}

#[test]
fn an_unusable_ca_bundle_fails_the_client() {
    let dir = tempfile::tempdir().expect("dir");
    let missing = dir.path().join("missing.pem");
    let error = Client::with_env(Settings::default(), bundle_env(&missing)).expect_err("missing");
    assert!(
        error
            .to_string()
            .starts_with(&format!("SSL_CERT_FILE={}: ", missing.display())),
        "{error}"
    );
    let empty = dir.path().join("empty.pem");
    std::fs::write(&empty, "no certificates here\n").expect("write");
    assert_eq!(
        Client::with_env(Settings::default(), bundle_env(&empty)).map(|_| ()),
        Err(Error::Config(format!(
            "SSL_CERT_FILE={}: the file holds no PEM certificate",
            empty.display()
        )))
    );
}
