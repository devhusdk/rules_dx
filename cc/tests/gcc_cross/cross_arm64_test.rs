use dx_testing::{image_machine, resolve_runfiles};

#[test]
fn gcc_arm64_artifacts_target_aarch64_glibc() {
    let paths = std::env::var("GCC_ARM64").expect("GCC_ARM64");
    let paths: Vec<&str> = paths.split_whitespace().collect();
    assert_eq!(paths.len(), 3, "expected three artifacts, got {paths:?}");
    for rel in paths {
        let path = resolve_runfiles(rel);
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
        assert!(
            !dx_testing::contains_bytes(&bytes, b"libstdc++.so"),
            "{rel}: links libstdc++ dynamically"
        );
    }
}
