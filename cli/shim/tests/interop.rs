mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_shim::fetch::{self, ChecksumSource, Request};

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

fn pins_text(fake: &[u8]) -> String {
    let sha = support::sha256_hex(fake);
    let manifest: String = [
        "dx-linux-arm64",
        "dx-linux-x86_64",
        "dx-macos-arm64",
        "dx-windows-arm64",
        "dx-windows-x86_64",
    ]
    .iter()
    .map(|asset| format!("{sha}  {asset}\n"))
    .collect();
    dx_adopt::LauncherPins::from_sha256sums(&manifest)
        .expect("pins")
        .to_lines(VERSION)
}

fn workspace(fake: &[u8]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".dx")).expect("mkdir");
    std::fs::create_dir_all(dir.path().join("tools")).expect("mkdir");
    std::fs::write(
        dir.path().join("MODULE.bazel"),
        format!("bazel_dep(name = \"rules_dx\", version = \"{VERSION}\")\n"),
    )
    .expect("module");
    let pins = pins_text(fake);
    if cfg!(windows) {
        std::fs::write(dir.path().join(".dx/pins"), pins.replace('\n', "\r\n")).expect("pin");
        std::fs::write(
            dir.path().join("tools/dx.cmd"),
            dx_adopt::render_launcher_cmd(),
        )
        .expect("launcher");
    } else {
        std::fs::write(dir.path().join(".dx/pins"), pins).expect("pin");
        std::fs::write(dir.path().join("tools/dx"), dx_adopt::render_launcher_sh())
            .expect("launcher");
    }
    dir
}

fn system_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{root}\\System32")
}

fn tools_dx(workspace: &Path, env: &[(&str, &Path)]) -> std::process::Output {
    let mut command = if cfg!(windows) {
        let mut command = Command::new(workspace.join("tools/dx.cmd"));
        command.arg("hello");
        command.env("PATH", system_path());
        command.env("USERPROFILE", "C:\\nonexistent-home");
        command
    } else {
        let mut command = Command::new("/bin/sh");
        command.arg(workspace.join("tools/dx")).arg("hello");
        command.env("PATH", "/usr/bin:/bin");
        command
    };
    command
        .env_remove("DX_CACHE_DIR")
        .env_remove("XDG_CACHE_HOME")
        .env("HOME", "/nonexistent-home");
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("run tools/dx")
}

#[test]
fn tools_dx_runs_a_binary_the_shim_downloaded() {
    let server = support::Server::start();
    let (asset, file) = support::host_asset();
    let fake = fake_dx();
    server.release(VERSION, &asset, &fake, &fake);
    let cache = tempfile::tempdir().expect("cache");
    let dest = dx_shim::protocol::bin_dir(cache.path(), VERSION).join(file);
    fetch::install(&Request {
        version: VERSION,
        asset: &asset,
        url: &server.url(VERSION, &asset),
        dest: &dest,
        checksum: &ChecksumSource::Sidecar,
    })
    .expect("shim installs");
    let ws = workspace(&fake);
    let output = tools_dx(ws.path(), &[("DX_CACHE_DIR", cache.path())]);
    assert!(
        String::from_utf8_lossy(&output.stdout).ends_with("arg=[hello]\n"),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let xdg = tempfile::tempdir().expect("xdg");
    let xdg_dest = dx_shim::protocol::bin_dir(&xdg.path().join("dx"), VERSION);
    std::fs::create_dir_all(&xdg_dest).expect("mkdir");
    std::fs::copy(&dest, xdg_dest.join(dest.file_name().expect("name"))).expect("copy");
    let output = tools_dx(ws.path(), &[("XDG_CACHE_HOME", xdg.path())]);
    assert!(String::from_utf8_lossy(&output.stdout).ends_with("arg=[hello]\n"));
}

#[test]
fn shim_runs_a_binary_in_the_tools_dx_layout() {
    let home = tempfile::tempdir().expect("home");
    let (_, file) = support::host_asset();
    let fake = fake_dx();
    let dir = home.path().join(".cache/dx/bin").join(VERSION);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join(file);
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        std::fs::canonicalize(std::env::var("DX_FAKE_BIN").expect("DX_FAKE_BIN")).expect("fake"),
        &bin,
    )
    .expect("seed");
    #[cfg(not(unix))]
    std::fs::write(&bin, &fake).expect("seed");
    let ws = workspace(&fake);
    let mut command = Command::new(shim_bin());
    command
        .arg("hello")
        .current_dir(ws.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home.path())
        .env("DX_RELEASE_BASE_URL", support::dead_base());
    if cfg!(windows) {
        command.env("USERPROFILE", home.path());
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    let output = command.output().expect("run shim");
    assert!(
        String::from_utf8_lossy(&output.stdout).ends_with("arg=[hello]\n"),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn shim_reads_the_pins_adopt_writes() {
    let (asset, _) = support::host_asset();
    let fake = fake_dx();
    let text = pins_text(&fake);
    assert_eq!(
        dx_shim::protocol::pinned_hash(&text, VERSION, &asset),
        Some(support::sha256_hex(&fake))
    );
    let crlf = text.replace('\n', "\r\n");
    assert_eq!(
        dx_shim::protocol::pinned_hash(&crlf, VERSION, &asset),
        Some(support::sha256_hex(&fake))
    );
}
