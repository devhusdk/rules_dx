//! The MSVC header and library subsets come from @msvc_runtime, not from copy actions (#8912).

#[test]
fn msvc_subsets_come_from_msvc_runtime() {
    let build = dx_testing::env_file("DX_MSVC_RUNTIMES_BUILD");
    let text = dx_testing::read_text(&build);
    let copies: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("copy_to_directory"))
        .collect();
    assert!(
        copies.is_empty(),
        "{} still copies MSVC files in build actions: {copies:?}",
        build.display()
    );
    assert!(
        text.contains("@msvc_runtime//:msvc_vcruntime_include"),
        "{} lacks @msvc_runtime//:msvc_vcruntime_include",
        build.display()
    );
}
