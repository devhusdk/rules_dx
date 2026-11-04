//! Runs the WASI task board under Wasmtime with only the declared capabilities.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn env(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set")))
}

fn run(args: &[&str], preopens: &[(&str, &str)]) -> std::process::Output {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let preopens: Vec<(String, PathBuf)> = preopens
        .iter()
        .map(|(guest, name)| (guest.to_string(), env(name)))
        .collect();
    wasi_run::run(
        &env("WASMTIME"),
        &env("TASKS_WASM"),
        &args,
        &BTreeMap::new(),
        &preopens,
    )
}

#[test]
fn board_runs_in_the_guest() {
    let output = run(
        &[
            "--asset=/assets/notes.txt",
            "/board/board.txt",
            "add Ship the WASI build",
            "done 2",
        ],
        &[("/assets", "NOTES"), ("/board", "BOARD")],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    let expected = [
        "ok:added 5",
        "ok:done 2",
        "[x] 1 Pin the Rust toolchain",
        "[x] 2 Draft release notes @notes.txt",
        "[ ] 3 Record emulator evidence",
        "[ ] 4 Ship the browser build",
        "[ ] 5 Ship the WASI build",
        "summary tasks=5 done=2 open=3 assets=1",
        "asset notes.txt bytes=41 adler32:3d6a0e9d",
    ]
    .join("\n");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim_end_matches(['\r', '\n']), expected);
}

#[test]
fn unknown_task_exits_1() {
    let output = run(&["/board/board.txt", "done 9"], &[("/board", "BOARD")]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("err:no_such_task: no task 9"), "{stderr}");
}

#[test]
fn board_is_unreadable_without_a_preopen() {
    let output = run(&["/board/board.txt"], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("err:unreadable: /board/board.txt"),
        "{stderr}"
    );
}
