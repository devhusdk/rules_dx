#![cfg(unix)]

mod support;

use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use support::Server;

const VERSION: &str = "1.2.3";

const FAKE_DX: &str = r#"#!/bin/sh
printf 'shim=%s\n' "${DX_SHIM:-}"
printf 'bazel=%s\n' "$(command -v bazel || echo none)"
for a in "$@"; do printf 'arg=[%s]\n' "$a"; done
case "${1:-}" in
  stdin) cat ;;
  exit) exit "$2" ;;
  kill) kill -TERM $$ ;;
esac
exit 0
"#;

fn shim_bin() -> PathBuf {
    let raw = std::env::var("DX_SHIM_BIN")
        .ok()
        .or_else(|| option_env!("CARGO_BIN_EXE_dx").map(str::to_owned))
        .expect("DX_SHIM_BIN");
    std::fs::canonicalize(raw).expect("shim binary")
}

struct Fixture {
    server: Server,
    cache: tempfile::TempDir,
    workspace: tempfile::TempDir,
    tools: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let server = Server::start();
        let (asset, _) = support::host_asset();
        server.release(VERSION, &asset, FAKE_DX.as_bytes(), FAKE_DX.as_bytes());
        let workspace = tempfile::tempdir().expect("workspace");
        std::fs::create_dir_all(workspace.path().join(".dx")).expect("mkdir");
        std::fs::write(
            workspace.path().join(".dx/version"),
            format!("{VERSION}\r\n"),
        )
        .expect("pin");
        let tools = tempfile::tempdir().expect("tools");
        let cat = ["/bin/cat", "/usr/bin/cat"]
            .into_iter()
            .map(Path::new)
            .find(|path| path.exists())
            .expect("cat");
        std::os::unix::fs::symlink(cat, tools.path().join("cat")).expect("link");
        Self {
            server,
            cache: tempfile::tempdir().expect("cache"),
            workspace,
            tools,
        }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(shim_bin());
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", self.tools.path())
            .env("HOME", self.cache.path())
            .env("DX_CACHE_DIR", self.cache.path())
            .env("DX_RELEASE_BASE_URL", &self.server.base);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(self.workspace.path(), args)
            .output()
            .expect("run shim")
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn passes_args_through_unchanged() {
    let fixture = Fixture::new();
    let args = ["a b", "\"quoted\"", "it's", "ünïcødé ✓", "", "--flag=x y"];
    let output = fixture.run(&args);
    assert!(output.status.success(), "{}", stderr(&output));
    let mut want = String::from("shim=1\nbazel=none\n");
    for arg in args {
        want.push_str(&format!("arg=[{arg}]\n"));
    }
    assert_eq!(stdout(&output), want);
    assert_eq!(stderr(&output), "");
}

#[test]
fn passes_non_utf8_args_through() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new();
    let raw = std::ffi::OsStr::from_bytes(b"\xff\xfe");
    let output = fixture
        .command(fixture.workspace.path(), &[])
        .arg(raw)
        .output()
        .expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.ends_with(b"arg=[\xff\xfe]\n"));
}

#[test]
fn inherits_stdin_and_stdout() {
    let fixture = Fixture::new();
    let mut child = fixture
        .command(fixture.workspace.path(), &["stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"piped input\n")
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    assert!(stdout(&output).ends_with("arg=[stdin]\npiped input\n"));
}

#[test]
fn exit_codes_pass_through() {
    let fixture = Fixture::new();
    for code in [0, 3, 42] {
        let output = fixture.run(&["exit", &code.to_string()]);
        assert_eq!(output.status.code(), Some(code));
    }
}

#[test]
fn signal_exit_passes_through() {
    let fixture = Fixture::new();
    let output = fixture.run(&["kill"]);
    assert_eq!(output.status.signal(), Some(15));
    let shell = Command::new("/bin/sh")
        .arg("-c")
        .arg("\"$0\" kill; echo $?")
        .arg(shim_bin())
        .current_dir(fixture.workspace.path())
        .env_clear()
        .env("PATH", fixture.tools.path())
        .env("DX_CACHE_DIR", fixture.cache.path())
        .env("DX_RELEASE_BASE_URL", &fixture.server.base)
        .output()
        .expect("sh");
    assert_eq!(stdout(&shell).lines().last(), Some("143"));
}

#[test]
fn second_run_is_offline() {
    let fixture = Fixture::new();
    assert!(fixture.run(&["one"]).status.success());
    let output = fixture
        .command(fixture.workspace.path(), &["two"])
        .env("DX_RELEASE_BASE_URL", support::dead_base())
        .output()
        .expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).ends_with("arg=[two]\n"));
}

#[test]
fn finds_the_pin_from_a_nested_directory() {
    let fixture = Fixture::new();
    let nested = fixture.workspace.path().join("a/b/c");
    std::fs::create_dir_all(&nested).expect("nested");
    let output = fixture.command(&nested, &["x"]).output().expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    let (_, file) = support::host_asset();
    assert!(fixture
        .cache
        .path()
        .join("bin")
        .join(VERSION)
        .join(file)
        .is_file());
}

#[test]
fn dx_version_env_overrides_the_file() {
    let fixture = Fixture::new();
    let (asset, _) = support::host_asset();
    fixture
        .server
        .release("4.5.6", &asset, FAKE_DX.as_bytes(), FAKE_DX.as_bytes());
    let output = fixture
        .command(fixture.workspace.path(), &[])
        .env("DX_VERSION", "4.5.6")
        .output()
        .expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.cache.path().join("bin/4.5.6").is_dir());
    assert!(!fixture.cache.path().join("bin").join(VERSION).exists());
}

#[test]
fn outside_a_workspace_fails_with_exit_1() {
    let fixture = Fixture::new();
    let outside = tempfile::tempdir().expect("outside");
    if dx_shim::protocol::find_version_file(outside.path()).is_some() {
        return;
    }
    let cwd = std::fs::canonicalize(outside.path()).expect("canonical");
    let output = fixture.command(&cwd, &[]).output().expect("run");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!(
            "dx: not in a dx workspace (no .dx/version above {}); run 'dx init' from a Bazel workspace with bazel run @rules_dx//:dx -- init --apply, or set DX_VERSION\n",
            cwd.display()
        )
    );
}

#[test]
fn bad_dx_version_is_a_usage_error() {
    let fixture = Fixture::new();
    let output = fixture
        .command(fixture.workspace.path(), &[])
        .env("DX_VERSION", "../../etc")
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with("dx: DX_VERSION must be a plain semver"));
    assert_eq!(fixture.server.hits(), 0);
}

#[test]
fn plain_http_mirror_is_refused() {
    let fixture = Fixture::new();
    let output = fixture
        .command(fixture.workspace.path(), &[])
        .env("DX_RELEASE_BASE_URL", "http://example.com/r")
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with("dx: DX_RELEASE_BASE_URL must use https://"));
}

#[test]
fn checksum_mismatch_exits_1_and_runs_nothing() {
    let fixture = Fixture::new();
    let (asset, file) = support::host_asset();
    fixture.server.release(
        VERSION,
        &asset,
        b"#!/bin/sh\necho pwned\n",
        FAKE_DX.as_bytes(),
    );
    let output = fixture.run(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).starts_with("dx: sha256 mismatch for "));
    let dir = fixture.cache.path().join("bin").join(VERSION);
    assert!(!dir.join(file).exists());
    assert!(support::leftovers(&dir).is_empty());
}

#[test]
fn concurrent_first_runs_share_one_download() {
    let fixture = Fixture::new();
    let children: Vec<_> = (0..4)
        .map(|i| {
            fixture
                .command(fixture.workspace.path(), &["exit", &i.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn")
        })
        .collect();
    for (i, child) in children.into_iter().enumerate() {
        let output = child.wait_with_output().expect("wait");
        assert_eq!(output.status.code(), Some(i as i32), "{}", stderr(&output));
    }
    let dir = fixture.cache.path().join("bin").join(VERSION);
    assert!(support::leftovers(&dir).is_empty());
    assert_eq!(std::fs::read_dir(&dir).expect("dir").count(), 1);
}

#[test]
fn shim_version_and_where() {
    let fixture = Fixture::new();
    let output = fixture.run(&["shim", "version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!("dx-shim {}\n", dx_shim::protocol::SHIM_VERSION)
    );
    let output = fixture.run(&["shim", "where"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.starts_with(&format!("version: {VERSION}\nsource: ")),
        "{text}"
    );
    assert!(text.ends_with("cached: no\n"), "{text}");
    assert_eq!(fixture.server.hits(), 0);
}

#[test]
fn unknown_shim_commands_are_usage_errors() {
    let fixture = Fixture::new();
    for (args, want) in [
        (
            &["shim", "self-update"][..],
            "dx: dx shim self-update is not available yet\n",
        ),
        (
            &["shim", "prune"],
            "dx: dx shim prune is not available yet\n",
        ),
        (
            &["shim", "bogus"],
            "dx: unknown dx shim command 'bogus'; use dx shim version or dx shim where\n",
        ),
        (
            &["shim"],
            "dx: dx shim needs a command: dx shim version or dx shim where\n",
        ),
        (
            &["shim", "version", "--check"],
            "dx: dx shim version takes no arguments\n",
        ),
    ] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert_eq!(stderr(&output), want);
    }
}
