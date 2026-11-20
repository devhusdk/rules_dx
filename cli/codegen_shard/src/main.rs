#![deny(clippy::indexing_slicing, clippy::string_slice)]

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    std::process::exit(codegen_shard::writer::run_os_process(
        &std::env::args_os().skip(1).collect::<Vec<_>>(),
    ));
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
