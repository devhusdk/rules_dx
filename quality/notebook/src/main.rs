#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

// LCOV_EXCL_START - reason: thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = quality_notebook::run_cli(
        &args,
        &|path| std::fs::read(path),
        &mut |path, bytes| std::fs::write(path, bytes),
        &mut |line| println!("{line}"),
        &mut |line| eprintln!("{line}"),
    );
    std::process::exit(code);
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
