/// Runs `quality_notebook` on the process's stdio and file system and returns the exit code.
// LCOV_EXCL_START - reason: thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
pub fn run_process(args: Vec<String>) -> i32 {
    crate::run_cli(
        &args,
        &|path| std::fs::read(path),
        &mut |path, bytes| std::fs::write(path, bytes),
        &mut |line| println!("{line}"),
        &mut |line| eprintln!("{line}"),
    )
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
