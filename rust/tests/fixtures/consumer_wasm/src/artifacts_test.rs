const WASM_MAGIC: [u8; 8] = [0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00];

fn artifact(name: &str) -> Vec<u8> {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    let path = dx_testing::resolve_runfiles(&rel);
    std::fs::read(&path).unwrap_or_else(|_| panic!("cannot read {}", path.display()))
}

fn artifact_text(name: &str) -> String {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    let path = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("cannot read {}", path.display()))
}

fn assert_raw_module(name: &str) {
    let bytes = artifact(name);
    assert!(
        bytes.len() > 8 && bytes[..8] == WASM_MAGIC,
        "{name} is not a raw wasm module: {} bytes",
        bytes.len()
    );
}

#[test]
fn web_bundle_is_a_raw_module_with_bindings() {
    assert_raw_module("WEB_WASM");
    let js = artifact_text("WEB_JS");
    assert!(
        js.contains("wasm_triple"),
        "web glue does not export wasm_triple"
    );
    let dts = artifact_text("WEB_DTS");
    assert!(
        dts.contains("wasm_triple"),
        "web types do not declare wasm_triple"
    );
}

#[test]
fn node_bundle_is_a_raw_module_without_typescript() {
    assert_raw_module("NODE_WASM");
    let js = artifact_text("NODE_JS");
    assert!(
        js.contains("wasm_triple"),
        "node glue does not export wasm_triple"
    );
}
