use dx_testing::{image_machine, resolve_runfiles};

#[test]
fn swift_binary_targets_aarch64_glibc() {
    let rel = std::env::var("HELLO_ARM64").expect("HELLO_ARM64");
    let path = resolve_runfiles(&rel);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    assert_eq!(
        image_machine(&bytes).as_deref(),
        Some("aarch64"),
        "{rel}: machine"
    );
    assert!(
        dx_testing::contains_bytes(&bytes, b"/lib/ld-linux-aarch64.so.1"),
        "{rel}: missing the aarch64 glibc interpreter"
    );
}
