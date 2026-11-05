mod check;

#[test]
fn msvc_outputs_target_arm64() {
    check::check("MSVC_ARM64", "aarch64");
}
