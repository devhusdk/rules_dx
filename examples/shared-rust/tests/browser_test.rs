//! Loads the relocated release browser build in a real Firefox and drives it.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const APP: &str = "examples/shared-rust/app_release/app";
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

fn text(browser: &mut Browser, id: &str) -> String {
    browser
        .text(&format!(
            "return document.getElementById('{id}').textContent"
        ))
        .unwrap_or_else(|error| panic!("read #{id}: {error}"))
}

fn lines(browser: &mut Browser) -> Vec<String> {
    let joined = browser
        .text(
            "return Array.from(document.querySelectorAll('#tasks li')) \
             .map((item) => item.textContent).join('\\n')",
        )
        .expect("read the rendered tasks");
    joined.lines().map(str::to_owned).collect()
}

fn apply(browser: &mut Browser, command: &str, reply: &str) {
    browser
        .script(&format!(
            "document.getElementById('command').value = {command:?}; \
             document.getElementById('run').click(); \
             return true;"
        ))
        .expect("drive the command control");
    let wait = format!("return document.getElementById('reply').textContent === {reply:?}");
    assert!(
        wait_for(browser, &wait, 30).expect("poll the reply"),
        "{command:?} never replied {reply:?}, got {:?}",
        text(browser, "reply"),
    );
}

#[test]
fn relocated_release_app_renders_and_drives_the_shared_board() {
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
            "return document.getElementById('version').textContent !== 'loading'",
            30,
        )
        .expect("poll the shared board"),
        "the shared application never ran",
    );
    assert_eq!(
        lines(&mut browser),
        [
            "[x] 1 Pin the Rust toolchain",
            "[ ] 2 Draft release notes @notes.txt",
            "[ ] 3 Record emulator evidence",
            "[ ] 4 Ship the browser build",
        ],
    );
    assert_eq!(
        text(&mut browser, "summary"),
        "tasks=4 done=1 open=3 assets=1"
    );
    assert_eq!(
        text(&mut browser, "asset"),
        "asset notes.txt bytes=41 adler32:3d6a0e9d"
    );
    assert_eq!(text(&mut browser, "load-error"), "err:missing_marker");
    assert_eq!(text(&mut browser, "version"), "2.0.0");
    assert_eq!(text(&mut browser, "build"), "release");
    apply(&mut browser, "add Ship the browser release", "ok:added 5");
    apply(&mut browser, "done 2", "ok:done 2");
    apply(&mut browser, "done 9", "err:no_such_task");
    assert_eq!(
        lines(&mut browser),
        [
            "[x] 1 Pin the Rust toolchain",
            "[x] 2 Draft release notes @notes.txt",
            "[ ] 3 Record emulator evidence",
            "[ ] 4 Ship the browser build",
            "[ ] 5 Ship the browser release",
        ],
    );
    assert_eq!(
        text(&mut browser, "summary"),
        "tasks=5 done=2 open=3 assets=1"
    );
    let _ = TcpStream::connect(("127.0.0.1", server.port)).expect("server stays up");
}
