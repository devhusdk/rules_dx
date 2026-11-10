// LCOV_EXCL_START - reason: thin shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

fn main() {
    std::process::exit(collect_native::run_process(
        std::env::args().skip(1).collect(),
    ));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md
