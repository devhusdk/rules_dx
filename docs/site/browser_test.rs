//! Serves the rendered site and drives it in a real Firefox at every hosting base.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const FIREFOX: &str = "DX_FIREFOX";
const USER_SITE: &str = "DX_USER_SITE";
const DESKTOP: (u64, u64) = (1280, 900);
const PHONE: (u64, u64) = (390, 780);
const WIDE: (u64, u64) = (1600, 1000);

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
    let kind = if candidate.extension().is_some_and(|ext| ext == "html") {
        "text/html; charset=utf-8"
    } else {
        "application/octet-stream"
    };
    reply(&mut stream, 200, kind, &body)
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

fn user_site() -> PathBuf {
    let rel = std::env::var(USER_SITE).expect("DX_USER_SITE must name the rendered site");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_dir(), "not a directory: {}", path.display());
    path
}

fn browser_at(base: &str) -> Browser {
    let profile = dx_testing::mkscratch("firefox-profile").expect("a browser profile");
    Browser::start(&firefox_binary(), &profile)
        .unwrap_or_else(|error| panic!("start Firefox for the {base} base: {error}"))
}

fn search_query(browser: &mut Browser, term: &str) {
    browser.press("s").expect("focus the search bar");
    for character in term.chars() {
        browser
            .press(&character.to_string())
            .unwrap_or_else(|error| panic!("type {character:?}: {error}"));
    }
    assert!(
        wait_for(browser, "return document.querySelectorAll('#searchresults li').length > 0", 30)
            .unwrap_or_else(|error| panic!("poll the search results: {error}")),
        "searching for {term:?} returned no result"
    );
}

#[test]
fn the_site_navigates_searches_and_reaches_every_command_page_in_a_browser() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser.set_viewport(DESKTOP.0, DESKTOP.1).expect("size the viewport");
    browser.navigate(&server.url("/")).expect("load the landing page");

    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    assert_eq!(
        browser
            .text("return document.querySelector('main h1, #content h1').textContent.trim()")
            .expect("the landing heading"),
        "rules_dx"
    );
    assert!(
        browser
            .count("return document.querySelectorAll('#sidebar a').length")
            .expect("sidebar links")
            >= 30,
        "the sidebar must list the whole book"
    );

    let links = browser
        .script(
            "return Array.from(document.querySelectorAll('#sidebar a'))\
             .map(function (a) { return a.getAttribute('href'); })",
        )
        .expect("sidebar routes");
    let routes: Vec<String> = links
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .filter(|href| href.ends_with(".html"))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        routes.len() >= 30,
        "the sidebar names {} rendered pages",
        routes.len()
    );

    for route in &routes {
        browser.navigate(&server.url(&format!("/{route}"))).expect("load every route");
        let title = browser.text("return document.title").expect("a page title");
        assert!(!title.is_empty(), "{route} rendered no title");
        let broken = browser.count(
            "return document.querySelectorAll('img[src=\"\"]').length",
        );
        assert_eq!(broken.unwrap_or_default(), 0, "{route} has an empty image source");
    }

    browser.navigate(&server.url("/")).expect("return to the landing page");
    search_query(&mut browser, "scope");
    assert!(
        browser
            .text("return document.querySelector('#searchresults-header').textContent")
            .expect("the search summary")
            .contains("scope"),
        "the search summary must name the query"
    );
    let first = browser
        .text("return document.querySelector('#searchresults a').getAttribute('href')")
        .expect("the first result");
    assert!(first.ends_with(".html"), "a search hit must link a page: {first}");
    browser.navigate(&server.url(&format!("/{first}"))).expect("follow the search hit");
    assert!(
        browser
            .truthy("return document.querySelector('h1') !== null")
            .expect("a heading"),
        "a search hit must render a page"
    );
}

#[test]
fn keyboard_navigation_walks_the_book_forward_and_back() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser.set_viewport(DESKTOP.0, DESKTOP.1).expect("size the viewport");
    browser.navigate(&server.url("/")).expect("load the landing page");

    let landing = browser.text("return location.pathname").expect("the landing path");
    browser.press("\u{e014}").expect("press the right arrow");
    let forward = browser.text("return location.pathname").expect("the next path");
    assert_ne!(forward, landing, "the right arrow must open the next page");
    browser.press("\u{e012}").expect("press the left arrow");
    assert_eq!(
        browser.text("return location.pathname").expect("the back path"),
        landing,
        "the left arrow must return to the landing page"
    );
    browser.press("\u{e017}").expect("press the slash key");
    assert_eq!(
        browser
            .text("return document.activeElement.id")
            .expect("the focused element"),
        "searchbar",
        "the slash key must focus the search bar"
    );
}

#[test]
fn the_sidebar_toggle_hides_the_navigation_and_focus_is_visible() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser.set_viewport(DESKTOP.0, DESKTOP.1).expect("size the viewport");
    browser.navigate(&server.url("/")).expect("load the landing page");

    let shown = browser
        .text("return getComputedStyle(document.getElementById('sidebar')).display")
        .expect("the sidebar display");
    assert_ne!(shown, "none", "the sidebar must start open");
    browser
        .script("document.getElementById('sidebar-toggle').click(); return 1")
        .expect("toggle the sidebar");
    thread::sleep(Duration::from_millis(800));
    assert_eq!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar')).display")
            .expect("the hidden sidebar display"),
        "none",
        "the toggle must hide the navigation"
    );

    let contrast = browser
        .text("return getComputedStyle(document.body).getPropertyValue('--fg').trim()")
        .expect("the body foreground");
    assert!(!contrast.is_empty(), "the stylesheet must define the body foreground");
    assert!(
        browser
            .truthy(
                "return Array.from(document.styleSheets).some(function (sheet) {\
                   try { return Array.from(sheet.cssRules).some(function (rule) {\
                     return rule.cssText.indexOf(':focus') >= 0; }); } catch (error) { return false; }\
                 });",
            )
            .expect("the focus rule"),
        "the stylesheet must style keyboard focus"
    );
}

#[test]
fn the_same_build_works_from_the_project_subdirectory_base() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser.set_viewport(DESKTOP.0, DESKTOP.1).expect("size the viewport");
    browser.navigate(&server.url("/rules_dx/")).expect("load the landing page");

    assert_eq!(
        browser.text("return document.title").expect("a title"),
        "rules_dx - rules_dx"
    );
    let stylesheet = browser
        .truthy(
            "return Array.from(document.styleSheets).some(function (sheet) {\
               try { return Array.from(sheet.cssRules).length > 0; } catch (error) { return false; }\
             });",
        )
        .expect("the loaded stylesheet");
    assert!(stylesheet, "the stylesheet must load from /rules_dx/");

    browser
        .navigate(&server.url("/rules_dx/docs/cli/commands/docs.html"))
        .expect("load a nested page from the subdirectory base");
    assert_eq!(
        browser.text("return document.title").expect("a nested title"),
        "Docs - rules_dx"
    );
    assert!(
        browser
            .truthy("return getComputedStyle(document.querySelector('pre code')).color !== ''")
            .expect("a highlighted code block"),
        "the nested page must render highlighted code"
    );
    search_query(&mut browser, "build");
    let first = browser
        .text("return document.querySelector('#searchresults a').getAttribute('href')")
        .expect("the first result");
    assert!(
        first.contains("../../"),
        "a search hit on a nested page must stay inside the base: {first}"
    );
}

#[test]
fn a_phone_sized_viewport_collapses_the_navigation() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser.set_viewport(PHONE.0, PHONE.1).expect("size the viewport");
    browser.navigate(&server.url("/rules_dx/")).expect("load the landing page");

    let width = browser
        .text("return String(window.innerWidth)")
        .expect("the viewport width");
    assert!(
        width.parse::<u64>().is_ok_and(|value| value <= 500),
        "the phone viewport reports {width} CSS pixels"
    );
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar-toggle')).display")
            .expect("the toggle display"),
        "none",
        "a phone must still be able to open the navigation"
    );
    browser
        .script("document.getElementById('sidebar-toggle').click(); return 1")
        .expect("open the navigation on a phone");
    thread::sleep(Duration::from_millis(800));
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar')).display")
            .expect("the phone sidebar display"),
        "none",
        "the toggle must open the navigation on a phone"
    );
    let overflow = browser.count(
        "return document.documentElement.scrollWidth > window.innerWidth + 2 ? 1 : 0",
    );
    assert_eq!(
        overflow.unwrap_or_default(),
        0,
        "the phone layout must not scroll sideways"
    );
}

#[test]
fn a_wide_viewport_shows_the_navigation_and_the_search_bar() {
    let server = Server::start(user_site(), "rules_dx");
    let mut browser = browser_at("subdirectory");
    browser.set_viewport(WIDE.0, WIDE.1).expect("size the viewport");
    browser.navigate(&server.url("/rules_dx/")).expect("load the landing page");

    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('sidebar')).display")
            .expect("the wide sidebar display"),
        "none",
        "a wide viewport must show the navigation"
    );
    assert_ne!(
        browser
            .text("return getComputedStyle(document.getElementById('searchbar')).display")
            .expect("the search bar display"),
        "none",
        "a wide viewport must show the search bar"
    );
    let overflow = browser.count(
        "return document.documentElement.scrollWidth > window.innerWidth + 2 ? 1 : 0",
    );
    assert_eq!(overflow.unwrap_or_default(), 0, "a wide layout must not scroll sideways");
}

#[test]
fn the_browser_reports_a_page_it_cannot_load() {
    let server = Server::start(user_site(), "");
    let mut browser = browser_at("root");
    browser.navigate(&server.url("/no-such-page.html")).expect("request a missing page");
    let text = browser.text("return document.body.textContent").expect("the 404 body");
    assert!(
        text.contains("Not Found"),
        "an unknown route must render the server's refusal: {text}"
    );
}
