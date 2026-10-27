//! Serves a rendered docs site over loopback for the Firefox-driven site tests.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use dx_site_browser::{wait_for, Browser};

const FIREFOX: &str = "DX_FIREFOX";
const USER_SITE: &str = "DX_USER_SITE";

pub struct Server {
    port: u16,
    stop: Sender<()>,
}

impl Server {
    pub fn start(root: PathBuf, base: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let port = listener.local_addr().expect("local addr").port();
        let (stop, done) = channel();
        let trimmed = base.trim_matches('/');
        let prefix = if trimmed.is_empty() {
            "/".to_string()
        } else {
            format!("/{trimmed}/")
        };
        thread::spawn(move || {
            for stream in listener.incoming() {
                if done.try_recv().is_ok() {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let _ = serve(stream, &root, &prefix);
            }
        });
        Server { port, stop }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve(mut stream: TcpStream, root: &Path, prefix: &str) -> std::io::Result<()> {
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
    if !target.starts_with(prefix) {
        return reply(&mut stream, 404, "text/plain", b"Not Found");
    }
    let relative = target[prefix.len()..].to_string();
    let file = root.join(if relative.is_empty() {
        "index.html".to_string()
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

/// Returns the media type one served file needs, or the binary default.
///
/// Firefox refuses a stylesheet or a font whose media type is wrong, so the
/// preview serves the same types a static host does.
fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match extension {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "txt" | "md" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn reply(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Not Found" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

pub fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

pub fn user_site() -> PathBuf {
    let rel = std::env::var(USER_SITE).expect("DX_USER_SITE must name the rendered site");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

pub fn search_query(browser: &mut Browser, term: &str) {
    browser.press("s").expect("focus the search bar");
    for character in term.chars() {
        browser
            .press(&character.to_string())
            .unwrap_or_else(|error| panic!("type {character:?}: {error}"));
    }
    assert!(
        wait_for(
            browser,
            "return document.querySelectorAll('#searchresults li').length > 0",
            30
        )
        .unwrap_or_else(|error| panic!("poll the search results: {error}")),
        "searching for {term:?} returned no result"
    );
}
