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
pub mod args;
pub mod exec;
pub mod finalize;
pub mod generate;
pub mod plan;
pub mod platform;
pub mod reports;
pub mod resolve;
pub mod skew;
#[cfg(test)]
mod test_support;

pub use args::{ArgsError, Command, Invocation, OperationMode, ReportRequest};
pub use exec::{execute, Env};
pub use finalize::{
    finalize, FinalizeError, FinalizeInput, FAILURE_MISSING_FILE, FAILURE_UNREADABLE_FILE,
    FAILURE_WRITE_MISMATCH,
};
pub use generate::{
    ensure_mode, manifest_is_check, project, render_diff, text_lines, ProjectedFile,
    ProjectedManifest, IGNORED_IMPORT_CODE, IGNORED_IMPORT_LEVEL, IGNORED_IMPORT_MESSAGE,
};
pub use plan::{BuildPlan, CommandSpec};
pub use reports::{Destination, PlannedReport, ReportError, StandardFormat};
pub use resolve::{
    expand_codegen_roots, map_owners_to_tests, resolve, resolve_for_test, resolve_run,
    ProcessQueryRunner, QueryResult, QueryRunner, ResolveError, ResolvedScope,
};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
