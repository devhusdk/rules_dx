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
    AppliedFile, ApplyError, ApplyReport, FileSystem, HookError, HookRunner, NoHooks,
    RealFileSystem, apply_envelope,
};
pub use consensus::{ConsensusError, merge};
pub use envelope::{
    ENVELOPE_VERSION, Envelope, EnvelopeError, FileOperation, emit_envelope, parse_envelope,
};
pub use validators::{BLOCKED_EXTENSIONS, MAX_OPERATION_BYTES, ValidationError, validate};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
