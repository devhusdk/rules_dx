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

pub mod applier;
pub mod consensus;
pub mod envelope;
pub mod validators;

pub use applier::{
    apply_envelope, AppliedFile, ApplyError, ApplyReport, FileSystem, HookError, HookRunner,
    NoHooks, RealFileSystem,
};
pub use consensus::{merge, ConsensusError};
pub use envelope::{
    emit_envelope, parse_envelope, Envelope, EnvelopeError, FileOperation, ENVELOPE_VERSION,
};
pub use validators::{validate, ValidationError, BLOCKED_EXTENSIONS, MAX_OPERATION_BYTES};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
