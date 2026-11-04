#[test]
fn declared_env_arrives() {
    assert_eq!(std::env::var("DX_WASI_NOTE").as_deref(), Ok("declared"));
}

#[test]
fn custom_runtime_adds_its_env() {
    assert_eq!(
        std::env::var("DX_WASI_RUNTIME_NOTE").as_deref(),
        Ok("custom")
    );
}
