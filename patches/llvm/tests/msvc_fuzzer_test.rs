//! The MSVC ABI accepts fuzzer but not asan, stages clang_rt.fuzzer*, and builds libFuzzer against libc++ without mingw headers.

fn patch() -> Vec<String> {
    let path = std::env::args().nth(1).expect("the patch path argument");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{path}: {error}"))
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_patch_stages_fuzzer_for_the_msvc_abi() {
    let lines = patch();
    let has = |line: &str| lines.iter().any(|candidate| candidate == line);
    assert!(has("-    \"fuzzer\","));
    assert!(
        !["-", "+", " "]
            .iter()
            .any(|sign| has(&format!("{sign}    \"asan\","))),
        "the patch touches the asan entry"
    );
    assert!(has("+        if sanitizer != \"fuzzer\""));
    for runtime in ["clang_rt.fuzzer", "clang_rt.fuzzer_no_main", "clang_rt.fuzzer_interceptors"] {
        let quoted = format!("\"{runtime}\"");
        assert!(
            lines.iter().any(|line| line.contains(&quoted)),
            "{runtime} is missing"
        );
    }
}

#[test]
fn the_msvc_headers_have_libcxx_and_no_mingw() {
    let lines = patch();
    let at = lines
        .iter()
        .position(|line| line == "+        \":windows_msvc\": [")
        .expect("the windows_msvc entry");
    let block = &lines[at..(at + 3).min(lines.len())];
    assert!(
        block.iter().any(|line| line == "+            \"@llvm-project//libcxx:headers\","),
        "{block:?}"
    );
    assert!(
        !block.iter().any(|line| line.contains("mingw")),
        "the patch gives libFuzzer mingw headers on the MSVC ABI"
    );
}
