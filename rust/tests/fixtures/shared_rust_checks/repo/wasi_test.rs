//! Runs the consumer report under Wasmtime.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn report_runs_in_the_guest() {
    let args = vec!["/board/board.txt".to_owned(), "done 1".to_owned()];
    let board = PathBuf::from(std::env::var("BOARD").unwrap());
    let preopens = vec![("/board".to_owned(), board)];
    let output = wasi_run::run(
        &PathBuf::from(std::env::var("WASMTIME").unwrap()),
        &PathBuf::from(std::env::var("REPORT_WASM").unwrap()),
        &args,
        &BTreeMap::new(),
        &preopens,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    let expected = [
        "id,done,title",
        "1,true,Review the shared core",
        "2,true,Wire the consumer app",
        "summary tasks=2 done=2 open=0 assets=1",
        "runtime=default",
    ]
    .join("\n");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim_end_matches(['\r', '\n']), expected);
}
