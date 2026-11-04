//! Runs the consumer report under Wasmtime.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn report_runs_in_the_guest() {
    let output = wasi_run::run(
        &PathBuf::from(std::env::var("WASMTIME").unwrap()),
        &PathBuf::from(std::env::var("REPORT_WASM").unwrap()),
        &[],
        &BTreeMap::new(),
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    let expected = [
        "ok:done 2",
        "summary tasks=2 done=2 open=0 assets=0",
        "core=2.0.0 toolchain=consumer",
    ]
    .join("\n");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim_end_matches(['\r', '\n']), expected);
}
