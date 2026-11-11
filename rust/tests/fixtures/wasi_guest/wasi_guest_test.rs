#[test]
fn ambient_environment_is_denied() {
    assert!(std::env::var_os("TEST_TMPDIR").is_none());
}

#[test]
fn declared_environment_arrives() {
    assert_eq!(std::env::var("DX_WASI_DECLARED").as_deref(), Ok("declared"));
}

#[test]
fn preopened_file_reads_back() {
    assert_eq!(
        std::fs::read_to_string("/data/input.txt").unwrap(),
        "staged input\n"
    );
}
