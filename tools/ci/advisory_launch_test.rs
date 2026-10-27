use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_testing::{read_runfiles, resolve_runfiles};

const WORKFLOW: &str = ".github/workflows/reusable-consumer.yml";

const SHIM: &str = r#"#!/usr/bin/env bash
shift
while [ $# -gt 0 ]; do
  case "$1" in
    @rules_dx//cli/advisory_prep) shift; break ;;
    *) shift ;;
  esac
done
if [ "${1:-}" != "--" ]; then
  echo "ERROR: Unrecognized option: ${1:-}" >&2
  exit 2
fi
shift
cd "$SHIM_RUNFILES"
exec "$SHIM_PREP" "$@" --archive "cargo=$SHIM_ARCHIVE"
"#;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn le16(value: usize) -> [u8; 2] {
    u16::try_from(value).expect("small").to_le_bytes()
}

fn le32(value: usize) -> [u8; 4] {
    u32::try_from(value).expect("small").to_le_bytes()
}

fn archive(name: &str, body: &str) -> Vec<u8> {
    let crc = crc32(body.as_bytes()).to_le_bytes();
    let mut out: Vec<u8> = Vec::new();
    out.extend(b"PK\x03\x04");
    out.extend(le16(20));
    out.extend([0u8; 8]);
    out.extend(crc);
    out.extend(le32(body.len()));
    out.extend(le32(body.len()));
    out.extend(le16(name.len()));
    out.extend(le16(0));
    out.extend(name.as_bytes());
    out.extend(body.as_bytes());
    let start = out.len();
    out.extend(b"PK\x01\x02");
    out.extend(le16(20));
    out.extend(le16(20));
    out.extend([0u8; 8]);
    out.extend(crc);
    out.extend(le32(body.len()));
    out.extend(le32(body.len()));
    out.extend(le16(name.len()));
    out.extend([0u8; 12]);
    out.extend(le32(0));
    out.extend(name.as_bytes());
    let size = out.len() - start;
    out.extend(b"PK\x05\x06");
    out.extend([0u8; 4]);
    out.extend(le16(1));
    out.extend(le16(1));
    out.extend(le32(size));
    out.extend(le32(start));
    out.extend(le16(0));
    out
}

fn prepare_script() -> String {
    let document: serde_json::Value = serde_json::from_value(
        yaml_serde::from_str::<serde_json::Value>(&read_runfiles(WORKFLOW)).expect("valid YAML"),
    )
    .expect("json");
    let steps = document["jobs"]["advisory-snapshots"]["steps"]
        .as_array()
        .expect("steps")
        .clone();
    let step = steps
        .iter()
        .find(|step| step["id"] == "prepare")
        .expect("prepare step");
    step["run"].as_str().expect("run script").to_owned()
}

struct Launch {
    _scratch: dx_test_scratch::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    status: std::process::ExitStatus,
    stderr: String,
}

fn launch(archive_bytes: Option<Vec<u8>>) -> Launch {
    let root_tmp = dx_test_scratch::scratch("advisory-launch");
    let root = root_tmp.path().to_path_buf();
    let workspace = root.join("check out");
    let lock = workspace.join("rust/tests/fixtures/hello/Cargo.lock");
    fs::create_dir_all(lock.parent().expect("parent")).expect("lock dir");
    fs::write(&lock, "version = 4\n").expect("lock");
    let runfiles = root.join("runfiles tree");
    fs::create_dir_all(&runfiles).expect("runfiles");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let shim = bin.join("bazel");
    fs::write(&shim, SHIM).expect("shim");
    make_executable(&shim);
    let zip = root.join("cargo.zip");
    if let Some(bytes) = archive_bytes {
        fs::write(&zip, bytes).expect("archive");
    }
    let prep = resolve_runfiles(&std::env::var("DX_ADVISORY_PREP").expect("DX_ADVISORY_PREP"));
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg("-ec")
        .arg(prepare_script())
        .current_dir(&workspace)
        .env("PATH", path)
        .env("BAZEL_CONFIG", "")
        .env("BB_ARGS", "")
        .env("GITHUB_WORKSPACE", &workspace)
        .env("GITHUB_OUTPUT", root.join("github_output"))
        .env("SHIM_RUNFILES", &runfiles)
        .env("SHIM_PREP", prep)
        .env("SHIM_ARCHIVE", &zip)
        .output()
        .expect("runs bash");
    Launch {
        _scratch: root_tmp,
        root,
        workspace,
        status: output.status,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

#[test]
fn the_prepare_step_reads_the_checkout_lock_and_writes_the_checkout_output() {
    let advisory = r#"{"id":"GHSA-a","modified":"2026-01-01T00:00:00Z","affected":[{"package":{"ecosystem":"crates.io","name":"aaa"},"versions":["1.0.0"]}]}"#;
    let run = launch(Some(archive("all/GHSA-a.json", advisory)));
    assert!(run.status.success(), "{}", run.stderr);
    let out = run.workspace.join(".dx/advisory");
    assert!(out.join("cargo.json").is_file(), "{}", run.stderr);
    assert!(out.join("cargo.meta.json").is_file(), "{}", run.stderr);
    assert!(
        !run.root.join("runfiles tree/.dx").exists(),
        "snapshots must not land in the runfiles tree"
    );
}

#[test]
fn a_failed_preparation_fails_the_step() {
    let run = launch(None);
    assert!(!run.status.success(), "{}", run.stderr);
    assert!(
        !run.workspace.join(".dx/advisory/cargo.json").exists(),
        "a failed preparation writes no snapshot"
    );
}
