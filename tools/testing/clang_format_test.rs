#[test]
fn sources_match_the_pinned_clang_format() {
    let tool = dx_testing::resolve_runfiles(
        &std::env::var("DX_CLANG_FORMAT").expect("DX_CLANG_FORMAT names the formatter"),
    );
    let style = dx_testing::resolve_runfiles(
        &std::env::var("DX_CLANG_FORMAT_STYLE").expect("DX_CLANG_FORMAT_STYLE names .clang-format"),
    );
    let sources = std::env::var("DX_FORMAT_SRCS").expect("DX_FORMAT_SRCS lists the sources");
    let style_arg = format!("--style=file:{}", style.display());
    let mut failures = Vec::new();
    for rel in sources.split_whitespace() {
        let file = dx_testing::resolve_runfiles(rel);
        let file_arg = file.to_string_lossy().into_owned();
        let run = dx_testing::run(
            &tool,
            &["--dry-run", "--Werror", style_arg.as_str(), file_arg.as_str()],
            &[],
        )
        .expect("clang-format runs");
        if !run.status.success() {
            failures.push(format!("{rel}\n{}", run.stderr));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
