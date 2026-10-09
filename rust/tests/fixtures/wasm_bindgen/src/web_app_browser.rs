//! Serves the staged web application over real HTTP and drives it in Firefox.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use dx_site_browser::{wait_for, Browser};

const FIREFOX: &str = "DX_FIREFOX";
const WEB_APP: &str = "DX_WEB_APP";

struct Server {
    port: u16,
    stop: Sender<()>,
}

impl Server {
    fn start(root: PathBuf) -> Self {
        Self::try_start(root).expect("bind a loopback port")
    }

    fn try_start(root: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (stop, done) = channel();
        thread::spawn(move || {
            for stream in listener.incoming() {
                if done.try_recv().is_ok() {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let _ = serve(stream, &root);
            }
        });
        Ok(Server { port, stop })
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn get(&self, path: &str) -> Response {
        request(self.port, path)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

struct Response {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    fn content_type(&self) -> &str {
        self.headers
            .get("content-type")
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn serve(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
    }
    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    let relative = target.trim_start_matches('/');
    if relative.contains("..") {
        return reply(&mut stream, 404, "text/plain", b"Not Found");
    }
    let file = root.join(if relative.is_empty() {
        "index.html".to_string()
    } else {
        relative.to_string()
    });
    let candidate = if file.is_dir() {
        file.join("index.html")
    } else {
        file
    };
    if !candidate.starts_with(root) {
        return reply(&mut stream, 404, "text/plain", b"Not Found");
    }
    let Ok(body) = std::fs::read(&candidate) else {
        return reply(&mut stream, 404, "text/plain", b"Not Found");
    };
    reply(&mut stream, 200, content_type(&candidate), &body)
}

fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn reply(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn request(port: u16, path: &str) -> Response {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("connect to the preview server");
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").expect("send request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| at + 4)
        .unwrap_or(raw.len());
    let head = String::from_utf8_lossy(&raw[..split.min(raw.len())]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
    }
    Response {
        status,
        headers,
        body: raw[split.min(raw.len())..].to_vec(),
    }
}

fn staged_app() -> PathBuf {
    let rel = std::env::var(WEB_APP).expect("DX_WEB_APP must name the staged application");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create the relocation root");
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read a staged dir").flatten() {
            let path = entry.path();
            let target = dst.join(path.strip_prefix(src).expect("under the staged root"));
            if path.is_dir() {
                std::fs::create_dir_all(&target).expect("recreate a staged dir");
                stack.push(path);
            } else {
                std::fs::copy(&path, &target).expect("copy a staged file");
            }
        }
    }
}

fn relocate(tag: &str) -> PathBuf {
    let root = dx_testing::mkscratch(tag).expect("a relocation root");
    let target = root.join("app");
    copy_dir(&staged_app(), &target);
    assert_ne!(
        target.canonicalize().expect("canonical relocation"),
        staged_app().canonicalize().expect("canonical stage"),
        "the served copy must live outside the stage"
    );
    target
}

fn start_browser(tag: &str) -> Browser {
    let profile = dx_testing::mkscratch(tag).expect("a browser profile");
    Browser::start(&firefox_binary(), &profile).expect("start Firefox")
}

fn page_json(browser: &mut Browser) -> String {
    browser
        .text("return document.getElementById('dx-out').textContent")
        .expect("read the page outcome")
}

#[test]
fn the_staged_app_serves_every_file_with_hosting_media_types() {
    let root = relocate("web-app-relocated");
    let server = Server::start(root.clone());
    let staged = [("index.html", "text/html; charset=utf-8")];
    for (name, kind) in staged {
        let response = server.get(&format!("/{name}"));
        assert_eq!(response.status, 200, "{name} served {}", response.status);
        assert_eq!(response.content_type(), kind, "{name} has the wrong media type");
    }
    let js = entry(&root, "js");
    let response = server.get(&format!("/{js}"));
    assert_eq!(response.status, 200, "the entry module served {}", response.status);
    assert_eq!(response.content_type(), "text/javascript; charset=utf-8");
    let wasm = entry(&root, "wasm");
    let response = server.get(&format!("/{wasm}"));
    assert_eq!(response.status, 200, "the wasm module served {}", response.status);
    assert_eq!(
        response.content_type(),
        "application/wasm",
        "the wasm module needs its streaming media type"
    );
    let response = server.get("/assets/message.json");
    assert_eq!(response.status, 200, "the asset served {}", response.status);
    assert_eq!(response.content_type(), "application/json");
    assert!(
        response.text().contains("Hello from the staged assets!"),
        "the asset lost its bytes"
    );
    let index = server.get("/");
    assert!(
        !index.text().contains("bazel-out"),
        "the staged page references an absolute build path"
    );
    assert_eq!(server.get("/missing.wasm").status, 404, "a missing file must 404");
}

#[test]
fn the_relocated_app_runs_its_checks_in_a_browser() {
    let root = relocate("web-app-browser");
    let server = Server::start(root);
    let mut browser = start_browser("web-app-profile");
    browser.navigate(&server.url("/")).expect("load the staged page");
    assert!(
        wait_for(browser, "return window.dxWebReady === true", 60).expect("poll readiness"),
        "the staged page never reported ready"
    );
    assert_eq!(
        browser
            .text("return window.dxWebError || ''")
            .expect("read the page error"),
        "",
        "the staged page reported an error"
    );
    assert_eq!(
        browser.text("return document.title").expect("a page title"),
        "wasm hello"
    );
    let outcome = page_json(&mut browser);
    assert!(
        outcome.contains("\"exports\":[\"add\",\"add_via_snippet\",\"greet\"]"),
        "the page did not see the wasm exports: {outcome}"
    );
    assert!(
        outcome.contains("\"greet('dx')\":\"Hello from wasm, dx!\""),
        "greet did not run: {outcome}"
    );
    assert!(outcome.contains("\"add(40, 2)\":42"), "add did not run: {outcome}");
    assert!(
        outcome.contains("\"add_via_snippet(40, 2)\":42"),
        "the snippet call did not run: {outcome}"
    );
    assert!(
        outcome.contains("Hello from the staged assets!"),
        "the asset did not load: {outcome}"
    );
}

#[test]
fn a_missing_wasm_module_reports_a_page_error() {
    let root = relocate("web-app-broken");
    for dead in std::fs::read_dir(&root).expect("read the relocation").flatten() {
        let path = dead.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wasm") {
            std::fs::remove_file(&path).expect("drop the wasm module");
        }
    }
    let server = Server::start(root);
    assert_eq!(
        server.get("/index.html").status,
        200,
        "the shell must still serve"
    );
    let mut browser = start_browser("web-app-broken-profile");
    browser.navigate(&server.url("/")).expect("load the broken page");
    assert!(
        !wait_for(browser, "return window.dxWebReady === true", 20).expect("poll readiness"),
        "the page without its wasm module must never report ready"
    );
    let error = browser
        .text("return window.dxWebError || ''")
        .expect("read the page error");
    assert!(!error.is_empty(), "the page hid its missing module");
}

#[test]
fn a_bound_port_reports_its_collision() {
    let first = Server::start(relocate("web-app-port"));
    let busy = TcpListener::bind(("127.0.0.1", first.port)).expect("hold the same port");
    drop(busy);
    let held = TcpListener::bind(("127.0.0.1", first.port));
    assert!(held.is_err(), "the loopback port must still be held");
    drop(first);
}

fn entry(root: &Path, extension: &str) -> String {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(root).expect("read the staged root").flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            found.push(path.file_name().expect("a file name").to_string_lossy().into_owned());
        }
    }
    assert_eq!(found.len(), 1, "the stage holds {found:?} .{extension} files");
    found.pop().expect("one staged entry")
}
