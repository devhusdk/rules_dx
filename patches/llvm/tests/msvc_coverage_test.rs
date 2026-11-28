//! The MSVC ABI accepts coverage with LLVM coverage maps, stages clang_rt.profile, and keeps gcov maps unsupported.

fn patch() -> Vec<String> {
    dx_testing::read_text(&dx_testing::env_file("DX_MSVC_COVERAGE_PATCH"))
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_patch_supports_llvm_coverage_on_the_msvc_abi() {
    let lines = patch();
    let has = |line: &str| lines.iter().any(|candidate| candidate == line);
    assert!(has("-    \"coverage\","));
    assert!(has("-    \"llvm_coverage_map_format\","));
    assert!(has("+        \":llvm_coverage_map_format\","));
    assert!(has("+        \"//toolchain/features:coverage_stub\","));
    assert!(has(
        "+        \"//toolchain/args:llvm_coverage_map_format_compile_args\","
    ));
    assert!(
        !["-", "+"]
            .iter()
            .any(|sign| has(&format!("{sign}    \"gcc_coverage_map_format\","))),
        "the patch touches the gcc_coverage_map_format entry"
    );
}

#[test]
fn the_patch_stages_the_msvc_profile_runtime_under_coverage() {
    let lines = patch();
    assert!(
        lines.iter().any(|line| line
            == "+            \"//runtimes/compiler-rt:clang_rt.profile.static\": \"clang_rt.profile\","),
        "clang_rt.profile is missing"
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "+    values = {\"collect_code_coverage\": \"true\"},")
    );
}
