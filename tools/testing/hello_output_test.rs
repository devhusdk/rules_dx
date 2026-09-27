#[test]
fn hello_output_matches_fixture() {
    let rel = std::env::var("DX_HELLO_BIN").expect("DX_HELLO_BIN must name the fixture binary");
    let expected =
        std::env::var("DX_HELLO_EXPECTED").expect("DX_HELLO_EXPECTED must hold the fixture output");
    let expected_code: i32 = std::env::var("DX_HELLO_EXPECTED_CODE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let binary = dx_testing::resolve_runfiles(&rel);
    let output = std::process::Command::new(&binary)
        .output()
        .expect("fixture binary must execute");
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected exit code for {}",
        binary.display()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = stdout.trim_end_matches(['\r', '\n']);
    assert_eq!(actual, expected, "unexpected hello output");
}
