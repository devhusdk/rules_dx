//! Serves the independent consumer application and checks its staged product.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

const CONSUMER_SERVE: &str = "DX_CONSUMER_SERVE";

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
    fn start() -> Self {
        let rel = std::env::var(CONSUMER_SERVE).expect("serve env is set");
        let path = dx_testing::resolve_runfiles(&rel);
        let mut child = Command::new(&path)
            .arg("--port=0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the consumer server");
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
        assert!(
            line.starts_with("dx-web-serve ready "),
            "bad ready line {line:?}"
        );
        let port = ready_port(&line);
        Server { child, port }
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
    body: Vec<u8>,
}

impl Response {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn request(port: u16, path: &str) -> Response {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n"
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
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header line");
        if line.trim().is_empty() {
            break;
        }
    }
    let mut body = Vec::new();
    reader.read_to_end(&mut body).expect("response body");
    Response { status, body }
}

#[test]
fn consumer_index_serves_the_shared_product() {
    let server = Server::start();
    let index = request(server.port, "/");
    assert_eq!(index.status, 200, "consumer index must serve");
    assert!(
        index.text().contains("consumer-proof"),
        "consumer page lost its proof node"
    );
    assert!(
        index.text().contains("./shared_web.js"),
        "consumer page must load the shared entry"
    );
}

#[test]
fn consumer_wasm_and_asset_serve() {
    let server = Server::start();
    let entry = request(server.port, "/shared_web.js");
    assert_eq!(entry.status, 200, "shared entry must serve");
    assert!(
        entry.text().contains("greet"),
        "served entry lost the shared export"
    );
    let wasm = request(server.port, "/shared_web_bg.wasm");
    assert_eq!(wasm.status, 200, "shared wasm must serve");
    assert_eq!(&wasm.body[0..4], b"\0asm", "served wasm lost its magic");
    let asset = request(server.port, "/assets/data.txt");
    assert_eq!(asset.status, 200, "consumer asset must serve");
    assert_eq!(asset.text(), "consumer asset\n", "unexpected asset content");
}

#[test]
fn consumer_missing_asset_stays_visible() {
    let server = Server::start();
    let missing = request(server.port, "/missing-asset.txt");
    assert_eq!(missing.status, 404, "missing assets must stay visible");
}
