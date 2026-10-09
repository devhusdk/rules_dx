use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use dx_site_browser::{wait_for, Browser};

const FIREFOX: &str = "DX_FIREFOX";
const SITE: &str = "rust/tests/fixtures/wasm_bindgen";

struct Server {
    port: u16,
    stop: Sender<()>,
}

impl Server {
    fn start(root: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let port = listener.local_addr().expect("local addr").port();
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
        Server { port, stop }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        let _ = TcpStream::connect(("127.0.0.1", self.port));
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
    let file = root.join(if relative.is_empty() {
        "browser_index.html"
    } else {
        relative
    });
    let candidate = if file.is_dir() {
        file.join("index.html")
    } else {
        file
    };
    let Ok(body) = std::fs::read(&candidate) else {
        return reply(&mut stream, 404, "text/plain", b"Not Found");
    };
    reply(&mut stream, 200, content_type(&candidate), &body)
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
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

fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

fn site_root() -> PathBuf {
    let root = dx_testing::runfiles_root().join(SITE);
    assert!(
        root.join("browser_index.html").is_file(),
        "not a directory: {}",
        root.display()
    );
    root
}

#[test]
fn a_js_consumer_imports_the_adapted_bindings_and_runs_them_in_a_browser() {
    let server = Server::start(site_root());
    let profile = dx_testing::mkscratch("firefox-profile").expect("a browser profile");
    let mut browser = Browser::start(&firefox_binary(), &profile)
        .unwrap_or_else(|error| panic!("start Firefox: {error}"));
    browser
        .navigate(&server.url("/browser_index.html"))
        .expect("load the consumer page");
    assert!(
        wait_for(
            &mut browser,
            "return document.querySelector('#shout').textContent !== 'pending'",
            30
        )
        .unwrap_or_else(|error| panic!("poll the consumer result: {error}")),
        "the consumer module never wrote its result"
    );
    assert_eq!(
        browser
            .text("return document.querySelector('#greeting').textContent")
            .expect("the greeting"),
        "Hello from wasm, browser!"
    );
    assert_eq!(
        browser
            .text("return document.querySelector('#shout').textContent")
            .expect("the snippet shout"),
        "HELLO FROM WASM, BROWSER!"
    );
}
