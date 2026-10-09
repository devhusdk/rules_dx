//! Asserts the opt-in bundle layout, its settings, and its serving.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

const PACKAGE: &str = "javascript/tests/fixtures/web_app/bundle";
const SERVE: &str = "DX_WEB_SERVE";
const STUB: &str = "DX_STUB_BUNDLER";

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

fn staged(name: &str) -> PathBuf {
    let path = dx_testing::resolve_runfiles(&format!("{PACKAGE}/{name}"));
    assert!(path.exists(), "missing bundle file: {name}");
    path
}

#[test]
fn bundle_keeps_the_entry_name_with_banner() {
    let js = std::fs::read_to_string(staged("wasm_hello_web.js")).expect("readable bundle");
    assert!(js.contains("stub bundle"), "bundle banner is missing");
    assert!(js.contains("MODE=release"), "custom define is missing");
    assert!(js.contains("greet"), "bundle lost the entry");
}

#[test]
fn bundle_records_settings_in_meta() {
    let meta = std::fs::read_to_string(staged("meta.json")).expect("readable meta");
    assert!(meta.contains("\"minify\": true"), "minify flag is missing");
    assert!(meta.contains("MODE=release"), "define is missing");
    assert!(meta.contains("{\\\"mode"), "custom config is missing");
}

#[test]
fn bundle_serves_its_loader() {
    let html = std::fs::read_to_string(staged("index.html")).expect("readable loader");
    assert!(
        html.contains("./wasm_hello_web.js"),
        "loader must name the bundle"
    );
}

#[test]
fn bundle_carries_the_wasm() {
    let bytes = std::fs::read(staged("wasm_hello_web_bg.wasm")).expect("readable wasm");
    assert_eq!(&bytes[0..4], b"\0asm", "bundled wasm lost its magic");
}

#[test]
fn bundle_serves_over_http() {
    let root = staged("wasm_hello_web.js")
        .parent()
        .expect("parent")
        .to_path_buf();
    let rel = std::env::var(SERVE).expect("serve env is set");
    let path = dx_testing::resolve_runfiles(&rel);
    let root_arg = format!("--root={}", root.to_string_lossy());
    let mut child = Command::new(&path)
        .args(["--port=0", &root_arg])
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
            "bad ready line {line:?}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
    let port = ready_port(&line);
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    write!(
        stream,
        "GET /wasm_hello_web.js HTTP/1.1\r\nHost: local\r\nConnection: close\r\n\r\n"
    )
    .expect("send request");
    let mut body = String::new();
    BufReader::new(stream)
        .read_to_string(&mut body)
        .expect("read body");
    assert!(body.contains(" 200 OK\r\n"), "bundle status: {body}");
    assert!(
        body.contains("text/javascript"),
        "bundle media type: {body}"
    );
    assert!(
        body.contains("stub bundle"),
        "bundle banner over HTTP: {body}"
    );
    child.kill().expect("stop the server");
    child.wait().expect("reap the server");
}

#[test]
fn stub_failure_propagates() {
    let rel = std::env::var(STUB).expect("stub env is set");
    let path = dx_testing::resolve_runfiles(&rel);
    let output = Command::new(&path)
        .arg("--fail")
        .output()
        .expect("run the stub");
    assert!(!output.status.success(), "a failing bundler must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requested failure"),
        "stderr names the failure: {stderr}"
    );
}
