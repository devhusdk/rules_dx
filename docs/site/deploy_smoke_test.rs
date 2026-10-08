//! Smoke-tests one served docs site against the staged artifact bytes.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const FIREFOX: &str = "DX_FIREFOX";
const USER_SITE: &str = "DX_USER_SITE";
const DEPLOYED_URL: &str = "DX_PAGES_URL";
const READINESS_ATTEMPTS: u32 = 10;
const READINESS_PAUSE: Duration = Duration::from_secs(3);
const PROBE_TIMEOUT: u64 = 15;
const DESKTOP: (u64, u64) = (1280, 900);
const IDENTITY_ROUTES: [&str; 2] = ["index.html", "searchindex.js"];
const REPRESENTATIVE_ROUTES: [&str; 4] = [
    "docs/cli/commands/docs.html",
    "docs/github-ci.html",
    "examples/adopt-cpp/index.html",
    "404.html",
];

struct Server {
    port: u16,
    stop: Sender<()>,
}

impl Server {
    fn start(root: PathBuf, base: &str) -> Self {
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
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
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

fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

fn user_site() -> PathBuf {
    let rel = std::env::var(USER_SITE).expect("DX_USER_SITE must name the rendered site");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

struct Fetched {
    status: u32,
    content_type: String,
    body: String,
}

fn fetch(browser: &mut Browser, url: &str) -> Fetched {
    let encoded = dx_testing::serde_json::to_string(url).expect("encode one smoke URL");
    let source = format!(
        "window.__dxSmoke = null; \
         fetch({encoded}).then(function (response) {{ \
           return response.text().then(function (body) {{ \
             window.__dxSmoke = {{ status: response.status, \
               contentType: response.headers.get('content-type') || '', body: body }}; \
           }}); \
         }}).catch(function () {{ \
           window.__dxSmoke = {{ status: 0, contentType: '', body: '' }}; \
         }}); \
         return 1;"
    );
    browser
        .script(&source)
        .unwrap_or_else(|error| panic!("probe {url}: {error}"));
    let settled = wait_for(
        browser,
        "return typeof window.__dxSmoke === 'object' && window.__dxSmoke !== null",
        PROBE_TIMEOUT,
    )
    .unwrap_or_else(|error| panic!("poll {url}: {error}"));
    if !settled {
        return Fetched {
            status: 0,
            content_type: String::new(),
            body: String::new(),
        };
    }
    let status = browser
        .text("return String(window.__dxSmoke.status)")
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let content_type = browser
        .text("return window.__dxSmoke.contentType")
        .unwrap_or_default();
    let body = browser
        .text("return window.__dxSmoke.body")
        .unwrap_or_default();
    Fetched {
        status,
        content_type,
        body,
    }
}

fn waits_for(browser: &mut Browser, base: &str) -> bool {
    for attempt in 0..READINESS_ATTEMPTS {
        let probe = format!("{base}index.html");
        if browser.navigate(base).is_ok() && fetch(browser, &probe).status == 200 {
            return true;
        }
        if attempt + 1 < READINESS_ATTEMPTS {
            thread::sleep(READINESS_PAUSE);
        }
    }
    false
}

fn search_query(browser: &mut Browser, term: &str) {
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

fn smoke(browser: &mut Browser, base: &str, staged: &Path) {
    assert!(
        waits_for(browser, base),
        "the site at {base} answered no HTTP 200 within {READINESS_ATTEMPTS} attempts"
    );

    for route in IDENTITY_ROUTES {
        let url = format!("{base}{route}");
        let served = fetch(browser, &url);
        assert_eq!(served.status, 200, "{url} served status {}", served.status);
        let expected = std::fs::read_to_string(staged.join(route))
            .unwrap_or_else(|error| panic!("read the staged {route}: {error}"));
        assert_eq!(
            served.body, expected,
            "{url} differs from the staged artifact"
        );
    }

    let css_url = format!("{base}css/general.css");
    let css = fetch(browser, &css_url);
    assert_eq!(css.status, 200, "{css_url} served status {}", css.status);
    assert!(
        css.content_type.starts_with("text/css"),
        "{css_url} served content type {:?}",
        css.content_type
    );

    for route in REPRESENTATIVE_ROUTES {
        let url = format!("{base}{route}");
        let served = fetch(browser, &url);
        assert_eq!(served.status, 200, "{url} served status {}", served.status);
        assert!(!served.body.is_empty(), "{url} served an empty document");
        assert!(
            served.content_type.starts_with("text/html"),
            "{url} served content type {:?}",
            served.content_type
        );
    }

    browser
        .navigate(base)
        .unwrap_or_else(|error| panic!("load the landing page at {base}: {error}"));
    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    assert!(
        browser
            .count("return document.querySelectorAll('#sidebar a').length")
            .expect("sidebar links")
            > 0,
        "the sidebar at {base} lists no page"
    );
    let stylesheet = browser
        .truthy(
            "return Array.from(document.styleSheets).some(function (sheet) {\
               try { return Array.from(sheet.cssRules).length > 0; } catch (error) { return false; }\
             });",
        )
        .expect("the loaded stylesheet");
    assert!(stylesheet, "no stylesheet rule applies at {base}");

    search_query(browser, "scope");
    let hit = browser
        .text("return document.querySelector('#searchresults a').href")
        .expect("the first search hit");
    browser
        .navigate(&hit)
        .unwrap_or_else(|error| panic!("follow the search hit {hit}: {error}"));
    let title = browser
        .text("return document.title")
        .expect("the search hit title");
    assert!(!title.is_empty(), "the search hit {hit} rendered no title");
}

#[test]
fn the_served_site_matches_the_staged_artifact_and_searches() {
    let staged = user_site();
    let mut browser = {
        let profile = dx_testing::mkscratch("firefox-profile").expect("a browser profile");
        Browser::start(&firefox_binary(), &profile)
            .unwrap_or_else(|error| panic!("start Firefox: {error}"))
    };
    browser
        .set_viewport(DESKTOP.0, DESKTOP.1)
        .expect("size the viewport");

    let server = Server::start(staged.clone(), "rules_dx");
    let local = format!("http://127.0.0.1:{}/rules_dx/", server.port);
    smoke(&mut browser, &local, &staged);

    let deployed = match std::env::var(DEPLOYED_URL) {
        Ok(value) => value,
        Err(_) => return,
    };
    assert!(
        !deployed.is_empty(),
        "{DEPLOYED_URL} is set but names no deployment URL"
    );
    let base = if deployed.ends_with('/') {
        deployed
    } else {
        format!("{deployed}/")
    };
    smoke(&mut browser, &base, &staged);
}
