#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use dx_lcov::run;

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(
        &args,
        &|path| {
            std::fs::read_to_string(path).map_err(|err| dx_lcov::LcovError::from(err.to_string()))
        },
        &mut |line| println!("{line}"),
    ));
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
