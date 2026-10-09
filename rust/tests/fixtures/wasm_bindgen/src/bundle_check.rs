use std::path::PathBuf;

fn bundle_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).expect("bundle env is set"))
}

#[test]
fn raw_wasm_has_module_magic() {
    let bytes = std::fs::read(bundle_path("DX_WASM_RAW")).expect("raw wasm is readable");
    assert!(bytes.len() > 8, "raw wasm is suspiciously small");
    assert_eq!(
        &bytes[0..4],
        b"\0asm",
        "raw output is missing the wasm magic"
    );
    assert_eq!(
        &bytes[4..8],
        &[1, 0, 0, 0],
        "raw output has an unexpected version"
    );
}

#[test]
fn bound_wasm_has_module_magic() {
    let bytes = std::fs::read(bundle_path("DX_WASM_BOUND")).expect("bound wasm is readable");
    assert!(bytes.len() > 8, "bound wasm is suspiciously small");
    assert_eq!(
        &bytes[0..4],
        b"\0asm",
        "bound output is missing the wasm magic"
    );
}

#[test]
fn web_js_exposes_greet() {
    let js = std::fs::read_to_string(bundle_path("DX_WASM_JS")).expect("web js is readable");
    assert!(js.contains("greet"), "web js does not expose greet");
}

#[test]
fn bare_js_exposes_greet() {
    let js = std::fs::read_to_string(bundle_path("DX_WASM_BARE_JS")).expect("bare js is readable");
    assert!(js.contains("greet"), "bare js does not expose greet");
}
