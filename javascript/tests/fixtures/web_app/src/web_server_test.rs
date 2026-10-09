//! Drives the managed development server over real HTTP and its negatives.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

const SERVE: &str = "DX_WEB_SERVE";
const SERVE_ISOLATED: &str = "DX_WEB_SERVE_ISOLATED";
const CONSUMER_SERVE: &str = "DX_WEB_CONSUMER_SERVE";

fn ready_port(line: &str) -> u16 {
    let url = line.split("url=").nth(1).expect("ready url");
    url.split(':')
        .nth(2)
        .expect("url port")
        .split('/')
        .next()
        .expect("port value")
        .parse()
        .expect("numeric port")
}

struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn start(env: &str, extra: &[&str]) -> Self {
        let rel = std::env::var(env).expect("serve env is set");
        let path = dx_testing::resolve_runfiles(&rel);
        let mut child = Command::new(&path)
            .arg("--port=0")
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the server");
        let stdout = child.stdout.take().expect("piped stdout");
        let (done, ready) = channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let outcome = BufReader::new(stdout).read_line(&mut line);
            let _ = done.send((outcome, line));
        });
        let (outcome, line) = ready
            .recv_timeout(Duration::from_secs(30))
            .expect("server ready");
        outcome.expect("read the ready line");
        if !line.starts_with("dx-web-serve ready ") {
            child.kill().expect("stop the failed server");
            let output = child.wait_with_output().expect("reap the failed server");
            panic!(
                "bad ready line {line:?} from {path:?}: {}",
                String::from_utf8_lossy(&output.stderr),
            );
        }
        let port = ready_port(&line);
        Server { child, port }
    }

    fn stop(self) {
        let mut server = self;
        server.child.kill().expect("stop the server");
        server.child.wait().expect("reap the server");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn request(port: u16, method: &str, path: &str) -> Response {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n"
    )
    .expect("send request");
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).expect("status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("numeric status");
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header line");
        let line = line.trim().to_string();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').expect("header pair");
        headers.push((name.trim().to_lowercase(), value.trim().to_string()));
    }
    let mut body = Vec::new();
    reader.read_to_end(&mut body).expect("response body");
    Response {
        status,
        headers,
        body,
    }
}

fn run_once(env: &str, extra: &[&str]) -> (Option<i32>, String) {
    let rel = std::env::var(env).expect("serve env is set");
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    let output = Command::new(&path)
        .args(extra)
        .output()
        .expect("run the server");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn serves_index_with_html_type() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/");
    assert_eq!(response.status, 200, "index status");
    assert_eq!(
        response.header("content-type"),
        Some("text/html; charset=utf-8"),
        "index media type",
    );
    assert_eq!(response.header("x-app"), Some("web"), "custom header");
    assert!(response.text().contains("wasm_hello_web.js"), "index body");
}

#[test]
fn serves_entry_with_javascript_type() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/wasm_hello_web.js");
    assert_eq!(response.status, 200, "entry status");
    assert_eq!(
        response.header("content-type"),
        Some("text/javascript; charset=utf-8"),
        "entry media type",
    );
    assert!(response.text().contains("greet"), "entry body");
}

#[test]
fn serves_wasm_with_wasm_type() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/wasm_hello_web_bg.wasm");
    assert_eq!(response.status, 200, "wasm status");
    assert_eq!(
        response.header("content-type"),
        Some("application/wasm"),
        "wasm media type"
    );
    assert_eq!(&response.body[0..4], b"\0asm", "wasm magic");
}

#[test]
fn serves_declared_asset() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/assets/data.txt");
    assert_eq!(response.status, 200, "asset status");
    assert_eq!(
        response.header("content-type"),
        Some("text/plain; charset=utf-8"),
        "asset media type",
    );
    assert_eq!(response.text(), "dx web asset\n", "asset body");
}

#[test]
fn missing_asset_is_404() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/missing.js");
    assert_eq!(response.status, 404, "missing status");
    assert_eq!(
        response.header("x-app"),
        Some("web"),
        "headers ride on errors too"
    );
}

#[test]
fn traversal_is_404() {
    let server = Server::start(SERVE, &[]);
    for path in ["/../secret", "/%2e%2e/secret", "/assets/../../secret"] {
        let response = request(server.port, "GET", path);
        assert_eq!(response.status, 404, "traversal {path} must not escape");
    }
}

#[test]
fn head_has_no_body() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "HEAD", "/");
    assert_eq!(response.status, 200, "head status");
    assert_eq!(
        response.header("content-type"),
        Some("text/html; charset=utf-8"),
        "head media type",
    );
    assert!(response.body.is_empty(), "head must not carry a body");
}

#[test]
fn rejects_write_methods() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "POST", "/");
    assert_eq!(response.status, 405, "post status");
    assert!(
        response.header("allow").unwrap_or_default().contains("GET"),
        "allow names reads",
    );
}

#[test]
fn isolated_server_sends_isolation_headers() {
    let server = Server::start(SERVE_ISOLATED, &[]);
    let response = request(server.port, "GET", "/");
    assert_eq!(response.status, 200, "isolated status");
    assert_eq!(
        response.header("cross-origin-opener-policy"),
        Some("same-origin"),
        "opener policy",
    );
    assert_eq!(
        response.header("cross-origin-embedder-policy"),
        Some("require-corp"),
        "embedder policy",
    );
}

#[test]
fn plain_server_omits_isolation_headers() {
    let server = Server::start(SERVE, &[]);
    let response = request(server.port, "GET", "/");
    assert_eq!(
        response.header("cross-origin-opener-policy"),
        None,
        "no opener policy"
    );
    assert_eq!(
        response.header("cross-origin-embedder-policy"),
        None,
        "no embedder policy"
    );
}

#[test]
fn colliding_port_fails() {
    let held = TcpListener::bind(("127.0.0.1", 0)).expect("hold a port");
    let port = held.local_addr().expect("held port").port();
    let (code, stderr) = run_once(SERVE, &[&format!("--port={port}")]);
    assert_ne!(code, Some(0), "a colliding port must fail");
    assert!(
        stderr.contains("cannot bind"),
        "names the bind failure: {stderr}"
    );
}

#[test]
fn non_loopback_host_is_refused() {
    let (code, stderr) = run_once(SERVE, &["--host=0.0.0.0"]);
    assert_eq!(code, Some(2), "a public host must fail");
    assert!(
        stderr.contains("refuses non-loopback"),
        "names the refusal: {stderr}"
    );
}

#[test]
fn bad_port_fails() {
    let (code, stderr) = run_once(SERVE, &["--port=99999"]);
    assert_eq!(code, Some(2), "a bad port must fail");
    assert!(stderr.contains("bad port"), "names the port: {stderr}");
}

#[test]
fn missing_root_fails() {
    let (code, stderr) = run_once(SERVE, &["--root=/nonexistent-dx-web-root"]);
    assert_eq!(code, Some(2), "a missing root must fail");
    assert!(stderr.contains("no directory"), "names the root: {stderr}");
}

#[test]
fn missing_index_fails() {
    let empty = dx_testing::mkscratch("web-empty").expect("scratch dir");
    let root = empty.to_string_lossy().into_owned();
    let (code, stderr) = run_once(SERVE, &[&format!("--root={root}")]);
    assert_eq!(code, Some(2), "a missing index must fail");
    assert!(
        stderr.contains("no index.html"),
        "names the index: {stderr}"
    );
}

#[test]
fn missing_wasm_fails() {
    let broken = dx_testing::mkscratch("web-broken").expect("scratch dir");
    std::fs::write(broken.join("index.html"), "<html></html>").expect("write index");
    std::fs::write(
        broken.join("manifest.json"),
        "{\"entry\": \"nope.js\", \"wasm\": \"nope.wasm\", \"version\": 1}",
    )
    .expect("write manifest");
    let root = broken.to_string_lossy().into_owned();
    let (code, stderr) = run_once(SERVE, &[&format!("--root={root}")]);
    assert_eq!(code, Some(2), "missing staged outputs must fail");
    assert!(
        stderr.contains("missing nope.js"),
        "names the missing file: {stderr}"
    );
}

#[test]
fn shutdown_frees_the_port() {
    let server = Server::start(SERVE, &[]);
    let port = server.port;
    server.stop();
    assert!(
        TcpListener::bind(("127.0.0.1", port)).is_ok(),
        "no leaked server may hold port {port}",
    );
}

#[test]
fn consumer_overrides_flow_through() {
    let server = Server::start(CONSUMER_SERVE, &[]);
    let response = request(server.port, "GET", "/");
    assert_eq!(response.status, 200, "consumer index status");
    assert_eq!(
        response.header("x-consumer"),
        Some("web-consumer"),
        "consumer header override",
    );
    assert_eq!(
        response.header("x-app"),
        None,
        "consumer keeps its own headers"
    );
    assert!(response.text().contains("consumer-proof"), "consumer body");
}
