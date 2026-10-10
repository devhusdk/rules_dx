//! Loads the relocated shared-core application in a real Firefox and runs it.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const APP: &str = "examples/shared-rust/app";
const FIREFOX: &str = "DX_FIREFOX";
const SERVE: &str = "DX_SHARED_SERVE";

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

#[test]
fn relocated_app_runs_shared_core_in_a_browser() {
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
        .expect("poll the shared proof"),
        "the shared application never ran",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('proof').textContent")
            .expect("the shared proof"),
        "ok:14",
        "unexpected shared evaluation",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('failure').textContent")
            .expect("the shared failure"),
        "err:division_by_zero",
        "invalid input must stay visible as an error",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('version').textContent")
            .expect("the shared version"),
        "1.0.0",
        "unexpected shared version",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('asset').textContent")
            .expect("the staged asset"),
        "2 * (3 + 4)\n(10 - 4) / 3\n7 % 3",
        "unexpected asset output",
    );
    browser
        .script(
            "document.getElementById('input').value = '7 * 6'; \
             document.getElementById('run').click(); \
             return true;",
        )
        .expect("drive the shared control");
    assert!(
        wait_for(
            &mut browser,
            "return document.getElementById('proof').textContent === 'ok:42'",
            30,
        )
        .expect("poll the driven proof"),
        "the shared control never re-evaluated",
    );
    let _ = TcpStream::connect(("127.0.0.1", server.port)).expect("server stays up");
}
