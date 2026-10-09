//! Loads the relocated shared application in a real Firefox and runs it.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const APP: &str = "rust/tests/fixtures/shared_app/shared_browser";
const FIREFOX: &str = "DX_FIREFOX";
const SERVE: &str = "DX_WEB_SERVE";

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
    fn start(root: &Path) -> Self {
        let rel = std::env::var(SERVE).expect("serve env is set");
        let path = dx_testing::resolve_runfiles(&rel);
        let arg = format!("--root={}", root.to_string_lossy());
        let mut child = Command::new(&path)
            .args(["--port=0", &arg])
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
        let port = ready_port(&line);
        Server { child, port }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn copy_tree(source: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("create dest");
    let mut entries: Vec<_> = std::fs::read_dir(source)
        .expect("read source")
        .map(|entry| entry.expect("dir entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let target = dest.join(entry.file_name());
        if std::fs::metadata(entry.path())
            .expect("file metadata")
            .is_dir()
        {
            copy_tree(&entry.path(), &target);
        } else {
            let bytes = std::fs::read(entry.path()).expect("read file");
            std::fs::write(target, bytes).expect("write file");
        }
    }
}

fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

fn missing_status(browser: &mut Browser) -> String {
    browser
        .script(
            "return fetch('./missing-asset.txt')\
             .then(function (response) { return response.status; })\
             .catch(function () { return -1; })",
        )
        .expect("fetch a missing asset")
        .to_string()
}

#[test]
fn relocated_shared_app_runs_wasm_in_a_browser() {
    let root = dx_testing::resolve_runfiles(&format!("{APP}/index.html"));
    let root = root.parent().expect("app dir").to_path_buf();
    let relocated = dx_testing::mkscratch("shared-relocated").expect("scratch dir");
    copy_tree(&root, &relocated);
    let server = Server::start(&relocated);
    let profile = dx_testing::mkscratch("firefox-profile").expect("a browser profile");
    let mut browser =
        Browser::start(&firefox_binary(), &profile).expect("start Firefox for the shared app");
    browser.navigate(&server.url("/")).expect("load the app");
    assert!(
        wait_for(
            &mut browser,
            "return document.getElementById('proof').textContent !== 'loading'",
            30,
        )
        .expect("poll the wasm proof"),
        "the shared application never ran",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('proof').textContent")
            .expect("the wasm proof"),
        "Hello from shared core, browser!",
        "unexpected wasm output",
    );
    assert!(
        wait_for(
            &mut browser,
            "return document.getElementById('count').textContent !== 'loading'",
            30,
        )
        .expect("poll the wasm counter"),
        "the shared counter never rendered",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('count').textContent")
            .expect("the wasm counter"),
        "shared core count is 42",
        "unexpected counter output",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('asset').textContent")
            .expect("the staged asset"),
        "shared browser asset",
        "unexpected asset output",
    );
    assert_eq!(
        missing_status(&mut browser),
        "404",
        "missing assets stay visible"
    );
    let _ = TcpStream::connect(("127.0.0.1", server.port)).expect("server stays up");
}
