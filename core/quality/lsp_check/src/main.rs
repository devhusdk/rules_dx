#![deny(clippy::indexing_slicing, clippy::string_slice)]

// LCOV_EXCL_START - reason: thin shim, issue: 6553, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    std::process::exit(lsp_check::entry::run_process(
        std::env::args().skip(1).collect(),
    ));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 6553, policy: docs/cli/commands/build-test-coverage.md
