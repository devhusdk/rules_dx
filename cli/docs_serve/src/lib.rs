#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Serves a built docs site over HTTP with nothing but the standard library.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL: Duration = Duration::from_millis(50);

/// A bound listener that serves one site directory under a base path.
pub struct Site {
    listener: TcpListener,
    root: PathBuf,
    prefix: String,
}

/// Keeps a spawned server running; dropping it stops the server.
pub struct Handle {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Site {
    /// Binds `host:port` and serves `root` under `base` (`""` serves at `/`).
    pub fn bind(host: &str, port: u16, root: PathBuf, base: &str) -> io::Result<Site> {
        let listener = TcpListener::bind((host, port))?;
        let trimmed = base.trim_matches('/');
        let prefix = if trimmed.is_empty() {
            "/".to_owned()
        } else {
            format!("/{trimmed}/")
        };
        Ok(Site {
            listener,
            root,
            prefix,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Serves until `stop` returns true, checked between accepts.
    pub fn serve_until(self, stop: &dyn Fn() -> bool) -> io::Result<()> {
        let root: Arc<Path> = Arc::from(self.root);
        let prefix: Arc<str> = Arc::from(self.prefix);
        self.listener.set_nonblocking(true)?;
        while !stop() {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    spawn_connection(stream, &root, &prefix);
                }
                Err(_) => thread::sleep(ACCEPT_POLL),
            }
        }
        Ok(())
    }

    /// Serves on a background thread until the returned handle is dropped.
    pub fn spawn(self) -> io::Result<Handle> {
        let addr = self.listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let root: Arc<Path> = Arc::from(self.root);
            let prefix: Arc<str> = Arc::from(self.prefix);
            for stream in self.listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                spawn_connection(stream, &root, &prefix);
            }
        });
        Ok(Handle {
            addr,
            stop,
            thread: Some(thread),
        })
    }
}

impl Handle {
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let wake = SocketAddr::new(loopback_for(&self.addr), self.addr.port());
        let _ = TcpStream::connect_timeout(&wake, READ_TIMEOUT);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn loopback_for(addr: &SocketAddr) -> std::net::IpAddr {
    if addr.ip().is_unspecified() {
        match addr {
            SocketAddr::V4(_) => std::net::Ipv4Addr::LOCALHOST.into(),
            SocketAddr::V6(_) => std::net::Ipv6Addr::LOCALHOST.into(),
        }
    } else {
        addr.ip()
    }
}

fn spawn_connection(stream: TcpStream, root: &Arc<Path>, prefix: &Arc<str>) {
    let root = Arc::clone(root);
    let prefix = Arc::clone(prefix);
    thread::spawn(move || {
        let _ = serve(stream, &root, &prefix);
    });
}

fn serve(mut stream: TcpStream, root: &Path, prefix: &str) -> io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
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
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" && method != "HEAD" {
        return reply(&mut stream, 405, "text/plain", b"Method Not Allowed", true);
    }
    let with_body = method == "GET";
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let Some(file) = resolve(root, prefix, path) else {
        return reply(&mut stream, 404, "text/plain", b"Not Found", with_body);
    };
    let candidate = if file.is_dir() {
        file.join("index.html")
    } else {
        file
    };
    let Ok(body) = std::fs::read(&candidate) else {
        return reply(&mut stream, 404, "text/plain", b"Not Found", with_body);
    };
    reply(&mut stream, 200, content_type(&candidate), &body, with_body)
}

/// Maps a request path to a file under `root`, or `None` when it must be refused.
fn resolve(root: &Path, prefix: &str, path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(path)?;
    let relative = decoded.strip_prefix(prefix)?;
    if relative.contains(['\\', '\0']) || relative.starts_with('/') {
        return None;
    }
    if relative.is_empty() {
        return Some(root.to_path_buf());
    }
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = text.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
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

fn reply(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
    with_body: bool,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        405 => "Method Not Allowed",
        _ => "Not Found",
    };
    let allow = if status == 405 {
        "Allow: GET, HEAD\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n{allow}Content-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    if with_body {
        stream.write_all(body)?;
    }
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};
    use std::io::Read;

    struct Response {
        status: u16,
        head: String,
        body: String,
    }

    fn fixture() -> impl AsRef<Path> {
        let dir = scratch("docs-serve");
        write(dir.as_ref(), "index.html", "<h1>home</h1>");
        write(dir.as_ref(), "guide/index.html", "<h1>guide</h1>");
        write(dir.as_ref(), "guide/my page.html", "spaced");
        write(dir.as_ref(), "css/site.css", "body {}");
        write(dir.as_ref(), "font/a.woff2", "font");
        dir
    }

    fn start(dir: &impl AsRef<Path>, base: &str) -> Handle {
        Site::bind("127.0.0.1", 0, dir.as_ref().to_path_buf(), base)
            .expect("bind")
            .spawn()
            .expect("spawn")
    }

    fn request(handle: &Handle, method: &str, path: &str) -> Response {
        let mut stream = TcpStream::connect(handle.local_addr()).expect("connect");
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: x\r\n\r\n").expect("send");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read");
        let text = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").expect("head and body");
        Response {
            status: head
                .split_whitespace()
                .nth(1)
                .expect("status")
                .parse()
                .expect("code"),
            head: head.to_owned(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn root_and_nested_directories_serve_their_index() {
        let dir = fixture();
        let server = start(&dir, "");
        assert_eq!(request(&server, "GET", "/").body, "<h1>home</h1>");
        assert_eq!(request(&server, "GET", "/guide/").body, "<h1>guide</h1>");
        assert_eq!(request(&server, "GET", "/guide").status, 200);
        assert_eq!(request(&server, "GET", "/missing.html").status, 404);
    }

    #[test]
    fn content_types_follow_the_extension() {
        let dir = fixture();
        let server = start(&dir, "");
        let css = request(&server, "GET", "/css/site.css");
        assert!(css.head.contains("Content-Type: text/css"), "{}", css.head);
        let font = request(&server, "GET", "/font/a.woff2");
        assert!(
            font.head.contains("Content-Type: font/woff2"),
            "{}",
            font.head
        );
        let page = request(&server, "GET", "/?q=1#top");
        assert!(page.head.contains("text/html"), "{}", page.head);
    }

    #[test]
    fn percent_escapes_decode() {
        let dir = fixture();
        let server = start(&dir, "");
        assert_eq!(
            request(&server, "GET", "/guide/my%20page.html").body,
            "spaced"
        );
        assert_eq!(request(&server, "GET", "/guide/%zz").status, 404);
    }

    #[test]
    fn paths_that_leave_the_root_are_refused() {
        let dir = fixture();
        let server = start(&dir, "");
        for path in [
            "/../",
            "/guide/../../x",
            "/%2e%2e/secret",
            "/guide/%2E%2E/%2E%2E/x",
            "//etc/passwd",
            "/..%5Csecret",
            "/guide%5C..%5Cindex.html",
        ] {
            assert_eq!(request(&server, "GET", path).status, 404, "{path}");
        }
    }

    #[test]
    fn only_get_and_head_are_allowed() {
        let dir = fixture();
        let server = start(&dir, "");
        let post = request(&server, "POST", "/");
        assert_eq!(post.status, 405);
        assert!(post.head.contains("Allow: GET, HEAD"), "{}", post.head);
    }

    #[test]
    fn head_has_headers_and_no_body() {
        let dir = fixture();
        let server = start(&dir, "");
        let head = request(&server, "HEAD", "/");
        assert_eq!(head.status, 200);
        assert!(head.head.contains("Content-Length: 13"), "{}", head.head);
        assert_eq!(head.body, "");
    }

    #[test]
    fn base_prefix_scopes_the_site() {
        let dir = fixture();
        let server = start(&dir, "/rules_dx/");
        assert_eq!(request(&server, "GET", "/rules_dx/").status, 200);
        assert_eq!(
            request(&server, "GET", "/rules_dx/guide/").body,
            "<h1>guide</h1>"
        );
        assert_eq!(request(&server, "GET", "/").status, 404);
        assert_eq!(request(&server, "GET", "/guide/").status, 404);
    }

    #[test]
    fn dropping_the_handle_stops_the_server() {
        let dir = fixture();
        let server = start(&dir, "");
        let addr = server.local_addr();
        drop(server);
        assert!(TcpStream::connect(addr).is_err());
    }

    #[test]
    fn serving_stops_once_asked() {
        let dir = fixture();
        let site = Site::bind("127.0.0.1", 0, dir.as_ref().to_path_buf(), "").expect("bind");
        let addr = site.local_addr().expect("addr");
        let stop = AtomicBool::new(false);
        let flag = &stop;
        thread::scope(|scope| {
            let server = scope.spawn(move || site.serve_until(&|| flag.load(Ordering::SeqCst)));
            let mut stream = TcpStream::connect(addr).expect("connect");
            write!(stream, "GET / HTTP/1.1\r\nHost: x\r\n\r\n").expect("send");
            let mut raw = String::new();
            stream.read_to_string(&mut raw).expect("read");
            assert!(raw.ends_with("<h1>home</h1>"), "{raw}");
            stop.store(true, Ordering::SeqCst);
            server.join().expect("server thread").expect("serve");
        });
    }

    #[test]
    fn binding_a_taken_port_fails() {
        let dir = fixture();
        let server = start(&dir, "");
        let port = server.local_addr().port();
        assert!(Site::bind("127.0.0.1", port, dir.as_ref().to_path_buf(), "").is_err());
    }
}
