use std::path::PathBuf;
use std::process::Command;

fn data_path(name: &str) -> PathBuf {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("missing env {name}"));
    if std::path::Path::new(&value).is_absolute() {
        return PathBuf::from(value);
    }
    let srcdir = std::env::var("TEST_SRCDIR").expect("missing TEST_SRCDIR");
    if value.starts_with("../") {
        return PathBuf::from(srcdir).join(value);
    }
    let workspace = std::env::var("TEST_WORKSPACE").expect("missing TEST_WORKSPACE");
    PathBuf::from(srcdir).join(workspace).join(value)
}

#[test]
fn old_cli_rejects_new_wasm() {
    let cli = data_path("DX_WASM_MISMATCH_CLI");
    let wasm = data_path("DX_WASM_GREET");
    let out = std::env::temp_dir().join("dx-wasm-mismatch");
    std::fs::create_dir_all(&out).expect("scratch dir");
    let result = Command::new(&cli)
        .arg("--target")
        .arg("web")
        .arg("--out-dir")
        .arg(&out)
        .arg("--out-name")
        .arg("mismatch")
        .arg(&wasm)
        .output()
        .unwrap_or_else(|_| panic!("run mismatch cli {} on {}", cli.display(), wasm.display()));
    assert!(
        !result.status.success(),
        "old cli unexpectedly accepted new wasm"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("0.2.100"), "stderr names the cli: {stderr}");
    assert!(stderr.contains("0.2.121"), "stderr names the lib: {stderr}");
}
