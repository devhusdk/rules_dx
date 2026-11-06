mod support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use support::{Reply, Server};

const VERSION: &str = "1.2.3";

fn fake_dx() -> Vec<u8> {
    std::fs::read(std::env::var("DX_FAKE_BIN").expect("DX_FAKE_BIN")).expect("fake dx")
}

fn shim_bin() -> PathBuf {
    let raw = std::env::var("DX_SHIM_BIN")
        .ok()
        .or_else(|| option_env!("CARGO_BIN_EXE_dx").map(str::to_owned))
        .expect("DX_SHIM_BIN");
    std::fs::canonicalize(raw).expect("shim binary")
}

fn module(version: &str) -> String {
    format!("module(name = \"app\")\nbazel_dep(name = \"rules_dx\", version = \"{version}\")\n")
}

struct Fixture {
    server: Server,
    cache: tempfile::TempDir,
    workspace: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let server = Server::start();
        let (asset, _) = support::host_asset();
        let fake = fake_dx();
        server.release(VERSION, &asset, &fake, &fake);
        let workspace = tempfile::tempdir().expect("workspace");
        std::fs::write(workspace.path().join("MODULE.bazel"), module(VERSION)).expect("module");
        std::fs::create_dir_all(workspace.path().join(".dx")).expect("mkdir");
        std::fs::write(
            workspace.path().join(".dx/pins"),
            format!("{VERSION} {} {asset}\r\n", support::sha256_hex(&fake)),
        )
        .expect("pin");
        Self {
            server,
            cache: tempfile::tempdir().expect("cache"),
            workspace,
        }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(shim_bin());
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("HOME", self.cache.path())
            .env("USERPROFILE", self.cache.path())
            .env("DX_CACHE_DIR", self.cache.path())
            .env("DX_RELEASE_BASE_URL", &self.server.base);
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
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
    let mut want = String::from("shim=1\n");
    for arg in args {
        want.push_str(&format!("arg=[{arg}]\n"));
    }
    assert_eq!(stdout(&output), want);
    assert_eq!(stderr(&output), "");
}

#[cfg(unix)]
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

#[cfg(unix)]
#[test]
fn signal_exit_passes_through() {
    use std::os::unix::process::ExitStatusExt;
    let fixture = Fixture::new();
    let output = fixture.run(&["kill"]);
    assert_eq!(output.status.signal(), Some(15));
    let shell = Command::new("/bin/sh")
        .arg("-c")
        .arg("\"$0\" kill; echo $?")
        .arg(shim_bin())
        .current_dir(fixture.workspace.path())
        .env_clear()
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
fn finds_module_bazel_from_a_nested_directory() {
    let fixture = Fixture::new();
    let nested = fixture.workspace.path().join("a/b/c");
    std::fs::create_dir_all(&nested).expect("nested");
    let output = fixture.command(&nested, &["x"]).output().expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    let (_, file) = support::host_asset();
    assert!(
        fixture
            .cache
            .path()
            .join("bin")
            .join(VERSION)
            .join(file)
            .is_file()
    );
}

#[test]
fn dx_version_env_overrides_module_bazel() {
    let fixture = Fixture::new();
    let (asset, _) = support::host_asset();
    let fake = fake_dx();
    fixture.server.release("4.5.6", &asset, &fake, &fake);
    let output = fixture
        .command(fixture.workspace.path(), &[])
        .env("DX_VERSION", "4.5.6")
        .output()
        .expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.cache.path().join("bin/4.5.6").is_dir());
    assert!(!fixture.cache.path().join("bin").join(VERSION).exists());
}

#[cfg(unix)]
impl Fixture {
    fn stub_bazel(&self) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().expect("bin");
        let log = bin.path().join("calls");
        let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display());
        let path = bin.path().join("bazel");
        std::fs::write(&path, script).expect("stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        (bin, log)
    }

    fn run_with_bazel(&self, bin: &Path, args: &[&str]) -> Output {
        self.command(self.workspace.path(), args)
            .env("PATH", bin)
            .output()
            .expect("run shim")
    }
}

#[cfg(unix)]
#[test]
fn bumping_only_the_bazel_dep_runs_bazel() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        module("1.2.4"),
    )
    .expect("bump");
    let (bin, log) = fixture.stub_bazel();
    let output = fixture.run_with_bazel(bin.path(), &["x", "y"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
    assert_eq!(
        std::fs::read_to_string(log).expect("log"),
        "run @rules_dx//:dx -- x y\n"
    );
    assert_eq!(fixture.server.hits(), 0);
}

#[cfg(unix)]
#[test]
fn an_override_or_a_missing_hash_runs_bazel() {
    let fixture = Fixture::new();
    let root = fixture.workspace.path();
    let (bin, log) = fixture.stub_bazel();
    std::fs::write(root.join(".dx/pins"), format!("{VERSION}\n")).expect("pins");
    assert!(fixture.run_with_bazel(bin.path(), &["a"]).status.success());
    std::fs::write(
        root.join("MODULE.bazel"),
        format!(
            "module(name = \"x\", module_name = \"rules_dx\")\n{}",
            module(VERSION)
        ),
    )
    .expect("override");
    assert!(fixture.run_with_bazel(bin.path(), &["b"]).status.success());
    let calls = std::fs::read_to_string(log).expect("log");
    assert_eq!(calls, "run @rules_dx//:dx -- a\nrun @rules_dx//:dx -- b\n");
    assert_eq!(fixture.server.hits(), 0);
}

#[cfg(unix)]
#[test]
fn a_multi_line_dep_runs_bazel() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        format!("bazel_dep(\n    name = \"rules_dx\",\n    version = \"{VERSION}\",\n)\n"),
    )
    .expect("module");
    let (bin, log) = fixture.stub_bazel();
    let output = fixture.run_with_bazel(bin.path(), &["m"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    assert_eq!(
        std::fs::read_to_string(log).expect("log"),
        "run @rules_dx//:dx -- m\n"
    );
    assert_eq!(fixture.server.hits(), 0);
}

#[cfg(unix)]
#[test]
fn dx_version_env_still_uses_the_sidecar() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.workspace.path().join("MODULE.bazel")).expect("rm");
    let output = fixture
        .command(fixture.workspace.path(), &["x"])
        .env("DX_VERSION", VERSION)
        .output()
        .expect("run");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.server.hits() >= 2);
    let output = fixture
        .command(fixture.workspace.path(), &["shim", "where"])
        .env("DX_VERSION", VERSION)
        .output()
        .expect("run");
    assert!(stdout(&output).contains("source: DX_VERSION\n"));
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
    fixture
        .server
        .release(VERSION, &asset, b"pwned", &fake_dx());
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
        text.starts_with(&format!("version: {VERSION}\nsource: MODULE.bazel\n")),
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

#[cfg(unix)]
#[test]
fn sigterm_mid_download_leaves_no_part_file() {
    let fixture = Fixture::new();
    let (asset, _) = support::host_asset();
    fixture
        .server
        .route(&format!("/v{VERSION}/{asset}"), Reply::Stall);
    let mut child = fixture
        .command(fixture.workspace.path(), &["--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn shim");
    let dir = fixture.cache.path().join("bin").join(VERSION);
    let has_part = || {
        std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".part"))
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !has_part() {
        assert!(std::time::Instant::now() < deadline, "no download started");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(143));
    assert!(!has_part());
}
