//! Serves the rendered site over real HTTP at every hosting base and walks it.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

const USER_SITE: &str = "DX_USER_SITE";

struct Server {
    port: u16,
    stop: Sender<()>,
}

impl Server {
    fn start(root: PathBuf, base: &'static str) -> Self {
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
                let _ = handle(stream, &root, &prefix);
            }
        });
        Server { port, stop }
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
    body: String,
}

impl Response {
    fn expect_ok(&self, path: &str) -> &Self {
        assert_eq!(self.status, 200, "{path} answered {}", self.status);
        assert!(!self.body.is_empty(), "{path} served an empty body");
        self
    }
}

fn handle(mut stream: TcpStream, root: &Path, prefix: &str) -> std::io::Result<()> {
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
    let _method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let path = target.split(['?', '#']).next().unwrap_or_default();
    if !path.starts_with(prefix) {
        return respond(&mut stream, 404, "Not Found");
    }
    let relative = path[prefix.len()..].to_string();
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
        return respond(&mut stream, 404, "Not Found");
    };
    let mut stream_out = Vec::new();
    stream_out.extend_from_slice(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    stream_out.extend_from_slice(&body);
    stream.write_all(&stream_out)?;
    stream.flush()
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

fn request(port: u16, path: &str) -> Response {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("connect to the preview server");
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").expect("send request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    Response {
        status,
        body: body.to_string(),
    }
}

fn site() -> PathBuf {
    let rel = std::env::var(USER_SITE).expect("DX_USER_SITE must name the rendered site");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

fn routes(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("read dir");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .expect("under root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out.sort();
    out
}

fn served_routes(root: &Path) -> Vec<String> {
    routes(root)
        .into_iter()
        .filter(|route| route != ".nojekyll")
        .collect()
}

#[test]
fn the_site_serves_every_page_and_asset_from_the_domain_root() {
    let root = site();
    let server = Server::start(root.clone(), "");
    for route in served_routes(&root) {
        let response = server.get(&format!("/{route}"));
        response.expect_ok(&route);
    }
    server.get("/").expect_ok("/");
    server.get("/index.html").expect_ok("/index.html");
    server
        .get("/docs/cli/commands/docs.html")
        .expect_ok("nested command page");
    assert_eq!(
        server.get("/nope.html").status,
        404,
        "an unknown route must 404"
    );
}

#[test]
fn the_same_build_serves_from_a_project_subdirectory() {
    let root = site();
    let server = Server::start(root.clone(), "rules_dx");
    server.get("/rules_dx/").expect_ok("subdirectory root");
    for route in served_routes(&root) {
        let response = server.get(&format!("/rules_dx/{route}"));
        response.expect_ok(&route);
    }
    assert_eq!(
        server.get("/docs/cli/commands/docs.html").status,
        404,
        "a route outside the base URL must not leak out of it"
    );
}

#[test]
fn a_deep_nested_command_page_resolves_its_own_relative_assets() {
    let root = site();
    let server = Server::start(root.clone(), "rules_dx");
    let page = server.get("/rules_dx/docs/cli/commands/docs.html");
    page.expect_ok("nested command page");
    for asset in [
        "../../../css/general.css",
        "../../../css/chrome.css",
        "../../../book.js",
        "../../../highlight.js",
        "../../../fonts/fonts.css",
    ] {
        let target = format!("/rules_dx/docs/cli/commands/{asset}");
        let response = server.get(&target);
        response.expect_ok(&target);
    }
}

#[test]
fn every_sidebar_and_page_link_resolves_over_http() {
    let root = site();
    let server = Server::start(root.clone(), "rules_dx");
    let pages = served_routes(&root)
        .into_iter()
        .filter(|route| route.ends_with(".html"));
    let mut visited: BTreeSet<String> = BTreeSet::new();
    for page in pages {
        let response = server.get(&format!("/rules_dx/{page}"));
        response.expect_ok(&page);
        for target in links(&response.body) {
            if target.starts_with("http") || target.starts_with('#') || target.starts_with("data:")
            {
                continue;
            }
            let clean = target.split('#').next().unwrap_or_default();
            if clean.is_empty() || clean == "./" || clean.starts_with('/') {
                continue;
            }
            let resolved = resolve(page.trim_start_matches('/'), clean);
            let url = format!("/rules_dx/{resolved}");
            let linked = server.get(&url);
            assert_eq!(
                linked.status, 200,
                "{page} links to {target}, which served {}",
                linked.status
            );
            visited.insert(resolved);
        }
    }
    assert!(
        visited.len() > 30,
        "the crawl only reached {} routes",
        visited.len()
    );
}

#[test]
fn the_search_index_is_served_at_both_bases() {
    let root = site();
    let at_root = Server::start(root.clone(), "");
    let at_sub = Server::start(root.clone(), "rules_dx");
    let body = at_root.get("/searchindex.js");
    body.expect_ok("search index");
    assert!(
        body.body.contains("window.search"),
        "the index is not the mdBook index"
    );
    let nested = at_sub.get("/rules_dx/searchindex.js");
    nested.expect_ok("search index");
    assert_eq!(
        nested.body, body.body,
        "the index must be identical at both bases"
    );
}

fn links(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("href=\"") {
        let tail = &rest[at + 6..];
        let Some((value, remainder)) = tail.split_once('"') else {
            break;
        };
        out.push(value.to_string());
        rest = remainder;
    }
    out
}

fn resolve(from: &str, target: &str) -> String {
    let base = from.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut segments: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}
