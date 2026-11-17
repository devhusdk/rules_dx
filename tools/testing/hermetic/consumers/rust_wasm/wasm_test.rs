//! Runs the WASI binary under Wasmtime and checks the browser bundle.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn wasi_binary_runs_in_the_guest() {
    let output = wasi_run::run(
        &PathBuf::from(std::env::var("WASMTIME").unwrap()),
        &PathBuf::from(std::env::var("WASI_WASM").unwrap()),
        &[],
        &BTreeMap::new(),
        &[],
    )
    .expect("the WASI runtime runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Hello from a Wasm consumer"
    );
}

#[test]
fn browser_bundle_has_module_and_bindings() {
    let files: Vec<PathBuf> = std::env::var("WEB_BUNDLE")
        .unwrap()
        .split(' ')
        .map(PathBuf::from)
        .collect();
    let wasm = files
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap_or_else(|| panic!("no .wasm in {files:?}"));
    assert!(std::fs::read(wasm).unwrap().starts_with(b"\0asm"));
    let js = files
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "js"))
        .unwrap_or_else(|| panic!("no .js in {files:?}"));
    assert!(std::fs::read_to_string(js).unwrap().contains("greeting"));
}
