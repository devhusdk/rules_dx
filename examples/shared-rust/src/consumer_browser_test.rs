//! Runs the independent shared-core consumer in a real Firefox.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;

use dx_site_browser::{wait_for, Browser};

const CONSUMER_SERVE: &str = "DX_SHARED_CONSUMER_SERVE";
const FIREFOX: &str = "DX_FIREFOX";

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
    fn start() -> Self {
        let rel = std::env::var(CONSUMER_SERVE).expect("consumer serve env is set");
        let path = dx_testing::resolve_runfiles(&rel);
        let mut child = Command::new(&path)
            .arg("--port=0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the consumer server");
        let stdout = child.stdout.take().expect("piped stdout");
        let (done, ready) = channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let outcome = BufReader::new(stdout).read_line(&mut line);
            let _ = done.send((outcome, line));
        });
        let (outcome, line) = ready
            .recv_timeout(Duration::from_secs(30))
            .expect("consumer server ready");
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

fn firefox_binary() -> PathBuf {
    let rel = std::env::var(FIREFOX).expect("DX_FIREFOX must name the acquired browser binary");
    let path = dx_testing::resolve_runfiles(&rel);
    assert!(path.is_file(), "not a file: {}", path.display());
    path
}

#[test]
fn independent_consumer_runs_shared_core_in_a_browser() {
    let server = Server::start();
    let profile = dx_testing::mkscratch("consumer-profile").expect("a browser profile");
    let mut browser =
        Browser::start(&firefox_binary(), &profile).expect("start Firefox for the consumer");
    browser
        .navigate(&server.url("/"))
        .expect("load the consumer");
    assert!(
        wait_for(
            &mut browser,
            "return document.getElementById('c-proof').textContent !== 'loading'",
            30,
        )
        .expect("poll the consumer proof"),
        "the consumer application never ran",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('c-proof').textContent")
            .expect("the consumer proof"),
        "ok:2",
        "unexpected consumer evaluation",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('c-failure').textContent")
            .expect("the consumer failure"),
        "err:division_by_zero",
        "consumer errors must stay visible",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('c-version').textContent")
            .expect("the consumer version"),
        "1.0.0",
        "unexpected consumer version",
    );
    assert_eq!(
        browser
            .text("return document.getElementById('c-asset').textContent")
            .expect("the consumer asset"),
        "1 + 2 * 3",
        "unexpected consumer asset",
    );
}
