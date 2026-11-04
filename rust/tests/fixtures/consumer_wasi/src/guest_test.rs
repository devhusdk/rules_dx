#[test]
fn builds_for_wasi_with_native_tools() {
    assert_eq!(consumer_wasi::module_family(), "wasm");
    assert_eq!(consumer_wasi::BUILD_TARGET, "wasm32-wasip1");
    assert_eq!(consumer_wasi::BUILD_SCRIPT_FAMILY, "native");
    assert_eq!(consumer_wasi::PROC_MACRO_FAMILY, "native");
    assert_eq!(consumer_wasi::env_or_unset("DX_WASI_WRAPPED"), "<unset>");
}
