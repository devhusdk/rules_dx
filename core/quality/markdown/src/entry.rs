/// Runs `quality_markdown` on the process's stdio and file system and returns the exit code.
// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub fn run_process(args: Vec<String>) -> i32 {
    dx_output::init_diagnostics(false);
    crate::run_cli(
        &args,
        &|path| {
            std::fs::read(path).map_err(|err| crate::MarkdownError::Io {
                message: err.to_string(),
            })
        },
        &mut |line| println!("{line}"),
        &mut |line| tracing::error!("{line}"),
    )
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
