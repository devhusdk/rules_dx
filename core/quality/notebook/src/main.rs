// LCOV_EXCL_START - reason: thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    std::process::exit(quality_notebook::entry::run_process(
        std::env::args().skip(1).collect(),
    ));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 2321, policy: docs/cli/commands/build-test-coverage.md
