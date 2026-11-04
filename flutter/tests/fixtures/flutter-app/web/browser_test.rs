//! Runs the relocated Flutter web bundle in Firefox with the network denied.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use dx_site_browser::{wait_for, Browser};

fn copy_tree(source: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("create dest");
    for entry in std::fs::read_dir(source).expect("read source") {
        let entry = entry.expect("dir entry");
        let target = dest.join(entry.file_name());
        if std::fs::metadata(entry.path()).expect("metadata").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html",
        Some("js" | "mjs") => "text/javascript",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn respond(mut stream: TcpStream, root: &Path) {
    let mut buffer = [0u8; 4096];
    let read = stream.read(&mut buffer).expect("read request");
    let request = String::from_utf8_lossy(&buffer[..read]);
    let target = request
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let relative = target.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let path = root.join(relative);
    let (status, body) = match std::fs::read(&path) {
        Ok(body) if !relative.contains("..") => ("200 OK", body),
        _ => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        content_type(&path),
        body.len(),
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

fn serve(root: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("local address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            std::thread::spawn(move || respond(stream, &root));
        }
    });
    port
}

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
    dx_testing::resolve_runfiles(&rel)
}

fn semantics_present(label: &str) -> String {
    format!(
        "return Array.from(document.querySelectorAll('flt-semantics')).some(function (node) {{ \
         return node.textContent === '{label}'; }})"
    )
}

#[test]
fn relocated_bundle_counts_offline() {
    let relocated = dx_testing::mkscratch("flutter-web").expect("scratch dir");
    let bundle = std::env::var("DX_BUNDLE").expect("DX_BUNDLE must be set");
    let bundle = bundle
        .split(' ')
        .find(|path| path.ends_with("_web"))
        .expect("the bundle directory");
    copy_tree(&dx_testing::resolve_runfiles(bundle), &relocated);
    let port = serve(relocated);
    let profile = dx_testing::mkscratch("firefox-profile").expect("browser profile");
    let mut browser = Browser::start(&runfile("DX_FIREFOX"), &profile).expect("start Firefox");
    browser
        .navigate(&format!("http://127.0.0.1:{port}/"))
        .expect("load the app");
    assert!(
        wait_for(&mut browser, &semantics_present("No items"), 30).expect("poll the app"),
        "the app never showed its initial count",
    );
    browser
        .script(
            "Array.from(document.querySelectorAll('flt-semantics')).find(function (node) { \
             return node.getAttribute('role') === 'button' && node.textContent === 'Add item'; }).click()",
        )
        .expect("click the add button");
    assert!(
        wait_for(&mut browser, &semantics_present("One item"), 20).expect("poll the count"),
        "the count did not change after the click",
    );
    let outside = browser
        .text(
            "return performance.getEntriesByType('resource').map(function (entry) { \
             return entry.name; }).filter(function (name) { \
             return new URL(name).origin !== location.origin; }).join(',')",
        )
        .expect("list requests");
    assert_eq!(outside, "", "requests left loopback");
}
