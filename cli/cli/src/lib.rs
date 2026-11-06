// LCOV_EXCL_START - reason: re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod adopt;
pub mod bazel_restart;
pub mod exec;
pub mod multicall;
pub mod startup;
#[cfg(test)]
mod test_support;

pub use exec::{ActionTally, Env, execute, print_actions};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
