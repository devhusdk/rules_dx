//! A browser binary with no deps bundles its module and bindings.

use std::path::PathBuf;

#[test]
fn implicit_bundle_has_module_and_bindings() {
    let files: Vec<PathBuf> = std::env::var("DX_WASM_IMPLICIT_BUNDLE")
        .expect("bundle env is set")
        .split(' ')
        .map(PathBuf::from)
        .collect();
    let wasm = files
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .unwrap_or_else(|| panic!("no .wasm in {files:?}"));
    assert!(
        std::fs::read(wasm)
            .expect("wasm is readable")
            .starts_with(b"\0asm")
    );
    let js = files
        .iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "js"))
        .unwrap_or_else(|| panic!("no .js in {files:?}"));
    assert!(
        std::fs::read_to_string(js)
            .expect("js is readable")
            .contains("implicit_greeting")
    );
}
