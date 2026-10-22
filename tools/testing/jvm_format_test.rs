#[test]
fn sources_match_the_pinned_formatter() {
    let tool = dx_testing::resolve_runfiles(
        &std::env::var("DX_FORMATTER").expect("DX_FORMATTER names the formatter"),
    );
    let sources = std::env::var("DX_FORMAT_SRCS").expect("DX_FORMAT_SRCS lists the sources");
    let extra = std::env::var("DX_FORMAT_ARGS").unwrap_or_default();
    let mut failures = Vec::new();
    for rel in sources.split_whitespace() {
        let file = dx_testing::resolve_runfiles(rel);
        let file_arg = file.to_string_lossy().into_owned();
        let mut args: Vec<&str> = extra.split_whitespace().collect();
        args.extend(["--dry-run", "--set-exit-if-changed", file_arg.as_str()]);
        let run = dx_testing::run(&tool, &args, &[]).expect("formatter runs");
        if !run.status.success() || !run.stdout.trim().is_empty() {
            failures.push(format!("{rel}\n{}", run.combined()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
