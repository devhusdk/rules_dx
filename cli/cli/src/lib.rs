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

mod adopt;
#[cfg(test)]
mod docs_parity_tests;
mod exec;

pub use exec::common::{emit_startup_outcome, flush_out};
pub use exec::{execute, hook_kind, run_worktree_hook, set_usage_command, Env, AUDIT_TODAY_ENV};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
