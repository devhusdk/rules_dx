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

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".dx")).expect("mkdir");
    std::fs::create_dir_all(dir.path().join("tools")).expect("mkdir");
    if cfg!(windows) {
        std::fs::write(dir.path().join(".dx/version"), format!("{VERSION}\r\n")).expect("pin");
        std::fs::write(
            dir.path().join("tools/dx.cmd"),
            dx_adopt::render_launcher_cmd(),
        )
        .expect("launcher");
    } else {
        std::fs::write(dir.path().join(".dx/version"), format!("{VERSION}\n")).expect("pin");
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
    let ws = workspace();
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
    let dir = home.path().join(".cache/dx/bin").join(VERSION);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let bin = dir.join(file);
    std::fs::write(&bin, fake_dx()).expect("seed");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let ws = workspace();
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
