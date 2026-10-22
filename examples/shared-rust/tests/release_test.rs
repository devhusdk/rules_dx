//! Relocates and runs the optimized native and WASI release artifacts.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const ASSETS: &str = "examples/shared-rust/assets";

const EXPECTED: &str = "ok:added 5
[x] 1 Pin the Rust toolchain
[ ] 2 Draft release notes @notes.txt
[ ] 3 Record emulator evidence
[ ] 4 Ship the browser build
[ ] 5 Ship the release
summary tasks=5 done=1 open=4 assets=1
asset notes.txt bytes=41 adler32:3d6a0e9d
";

fn artifact(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} names a release artifact"));
    dx_testing::resolve_runfiles(&rel)
}

fn stage(prefix: &str, artifact: &Path, name: &str) -> (PathBuf, PathBuf) {
    let dir = dx_testing::mkscratch(prefix).expect("scratch dir");
    let target = dir.join(name);
    std::fs::copy(artifact, &target).expect("relocate the artifact");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    for asset in ["board.txt", "notes.txt"] {
        let source = dx_testing::resolve_runfiles(&format!("{ASSETS}/{asset}"));
        std::fs::copy(source, dir.join(asset)).expect("relocate the asset");
    }
    (target, dir)
}

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn optimized_native_release_runs_relocated() {
    let _serial = serial();
    let (bin, dir) = stage("tasks-release", &artifact("DX_TASKS_RELEASE"), "tasks");
    let notes = format!("--asset={}", dir.join("notes.txt").display());
    let board = dir.join("board.txt");
    let result = dx_testing::run(
        &bin,
        &[
            &notes,
            board.to_str().expect("utf8 path"),
            "add Ship the release",
        ],
        &[],
    )
    .expect("run the release binary");
    assert!(result.status.success(), "stderr: {}", result.stderr);
    assert_eq!(result.stdout, EXPECTED);
    assert_eq!(result.stderr, "core:2.0.0 build=release\n");
}

#[test]
fn optimized_wasi_release_runs_relocated_under_wasmtime() {
    let _serial = serial();
    let (module, dir) = stage(
        "tasks-wasi-release",
        &artifact("DX_TASKS_WASI_RELEASE"),
        "tasks.wasm",
    );
    let bytes = std::fs::read(&module).expect("read the module");
    assert_eq!(&bytes[..4], b"\0asm", "release module is not Wasm");
    let preopen = format!("{}::/data", dir.display());
    let result = dx_testing::run(
        &artifact("DX_WASMTIME"),
        &[
            "run",
            "--dir",
            &preopen,
            module.to_str().expect("utf8 path"),
            "--asset=/data/notes.txt",
            "/data/board.txt",
            "add Ship the release",
        ],
        &[],
    )
    .expect("run the release module");
    assert!(result.status.success(), "stderr: {}", result.stderr);
    assert_eq!(result.stdout, EXPECTED);
    assert_eq!(result.stderr, "core:2.0.0 build=release\n");
}
