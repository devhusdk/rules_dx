#[test]
fn custom_runtime_injects_env() {
    assert_eq!(
        consumer_wasi::env_or_unset("DX_WASI_WRAPPED"),
        "wrapped-value"
    );
}
