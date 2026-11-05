use dx_testing::{image_machine, resolve_runfiles};
use std::process::Command;

#[test]
fn gcc_binary_built_and_run_on_native_arm64() {
    assert_eq!(std::env::consts::ARCH, "aarch64");
    let app = resolve_runfiles(&std::env::var("APP").expect("APP"));
    let bytes = std::fs::read(&app).expect("read app");
    assert_eq!(image_machine(&bytes).as_deref(), Some("aarch64"));

    let out = Command::new(&app).env_clear().output().expect("run app");
    assert!(out.status.success(), "app failed: {out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello from c");
}
