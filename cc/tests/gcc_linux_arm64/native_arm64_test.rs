#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
use dx_testing::env_file;
use dx_testing_image::machine;
use std::process::Command;

#[test]
fn gcc_binary_built_and_run_on_native_arm64() {
    assert_eq!(std::env::consts::ARCH, "aarch64");
    let app = env_file("APP");
    let bytes = std::fs::read(&app).expect("read app");
    assert_eq!(machine(&bytes).as_deref(), Some("aarch64"));

    let out = Command::new(&app).env_clear().output().expect("run app");
    assert!(out.status.success(), "app failed: {out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello from c");
}
