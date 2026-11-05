mod check;

#[test]
fn msvc_outputs_target_x64() {
    check::check("MSVC_X64", "x86_64");
}
