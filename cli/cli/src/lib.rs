#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    clippy::wildcard_enum_match_arm
)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

// LCOV_EXCL_START - reason: re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

pub mod adopt;
pub mod exec;
pub mod multicall;
pub mod startup;
#[cfg(test)]
mod test_support;

pub use exec::{ActionTally, Env, execute, print_actions, print_cache_retries};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
