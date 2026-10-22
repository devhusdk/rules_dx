//! Runs the relocated native task board over its packaged resources.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const ASSETS: &str = "examples/shared-rust/assets";

const EXPECTED: &str = "ok:added 5
ok:done 2
ok:attached notes.txt to 5
[x] 1 Pin the Rust toolchain
[x] 2 Draft release notes @notes.txt
[ ] 3 Record emulator evidence
[ ] 4 Ship the browser build
[ ] 5 Ship the native build @notes.txt
summary tasks=5 done=2 open=3 assets=2
asset notes.txt bytes=41 adler32:3d6a0e9d
";

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn relocated() -> (PathBuf, PathBuf) {
    let rel = std::env::var("DX_TASKS").expect("DX_TASKS names the native binary");
    let dir = dx_testing::mkscratch("tasks-native").expect("scratch dir");
    let bin = dir.join("tasks");
    std::fs::copy(dx_testing::resolve_runfiles(&rel), &bin).expect("relocate the binary");
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    for name in ["board.txt", "notes.txt"] {
        let source = dx_testing::resolve_runfiles(&format!("{ASSETS}/{name}"));
        std::fs::copy(source, dir.join(name)).expect("relocate the asset");
    }
    (bin, dir)
}

fn run(bin: &Path, args: &[&str]) -> dx_testing::Run {
    dx_testing::run(bin, args, &[]).expect("run the native board")
}

#[test]
fn relocated_binary_applies_commands_over_packaged_resources() {
    let _serial = serial();
    let (bin, dir) = relocated();
    let board = dir.join("board.txt");
    let notes = format!("--asset={}", dir.join("notes.txt").display());
    let result = run(
        &bin,
        &[
            &notes,
            board.to_str().expect("utf8 path"),
            "add Ship the native build",
            "done 2",
            "attach 5 notes.txt",
        ],
    );
    assert!(result.status.success(), "stderr: {}", result.stderr);
    assert_eq!(result.stdout, EXPECTED);
    assert_eq!(result.stderr, "core:2.0.0 build=dev\n");
}

#[test]
fn rejected_command_exits_one_with_its_kind() {
    let _serial = serial();
    let (bin, dir) = relocated();
    let board = dir.join("board.txt");
    let result = run(&bin, &[board.to_str().expect("utf8 path"), "launch 1"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stdout, "");
    assert_eq!(
        result.stderr,
        "err:unknown_command: unknown command \"launch\"\n"
    );
}

#[test]
fn malformed_or_missing_board_exits_two() {
    let _serial = serial();
    let (bin, dir) = relocated();
    let broken = dir.join("broken.txt");
    std::fs::write(&broken, "[x] fine\noops\n").expect("write broken board");
    let result = run(&bin, &[broken.to_str().expect("utf8 path")]);
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        result.stderr,
        "err:missing_marker: line 2 does not start with [ ] or [x]\n"
    );
    let missing = dir.join("missing.txt");
    let result = run(&bin, &[missing.to_str().expect("utf8 path")]);
    assert_eq!(result.status.code(), Some(2));
    assert!(
        result.stderr.starts_with("err:unreadable: "),
        "stderr: {}",
        result.stderr
    );
    let result = run(&bin, &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.starts_with("usage: tasks"));
}
