//! The Go toolchains hub declares one SDK per exec platform.

use std::collections::BTreeMap;

fn quoted<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.trim()
        .strip_prefix(key)?
        .strip_prefix(" = \"")?
        .strip_suffix("\",")
}

#[test]
fn each_exec_platform_has_one_sdk() {
    let build = dx_testing::env_file("DX_GO_TOOLCHAINS_BUILD");
    let text = dx_testing::read_text(&build);
    let mut sdks: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut repo = None;
    let mut goarch = None;
    for line in text.lines() {
        if let Some(value) = quoted(line, "go_toolchain_repo") {
            repo = Some(value.to_owned());
        } else if let Some(value) = quoted(line, "exec_goarch") {
            goarch = Some(value.to_owned());
        } else if let Some(goos) = quoted(line, "exec_goos")
            && let (Some(repo), Some(goarch)) = (repo.take(), goarch.take())
        {
            sdks.entry((goos.to_owned(), goarch))
                .or_default()
                .push(repo);
        }
    }
    assert!(!sdks.is_empty(), "{} declares no Go SDK", build.display());
    let shared: Vec<_> = sdks.iter().filter(|(_, repos)| repos.len() > 1).collect();
    assert!(
        shared.is_empty(),
        "{} registers several SDKs for one exec platform: {shared:?}",
        build.display()
    );
}
