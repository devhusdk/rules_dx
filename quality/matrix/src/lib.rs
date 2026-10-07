//! Layer-2 quality matrix harness: manifest in, verified snapshot result out.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod manifest;
pub mod run;
pub mod verify;

pub use manifest::{Manifest, MANIFEST_SCHEMA_VERSION};
pub use run::{execute, run, update_requested};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("no runfiles for this case: {0}")]
    Runfiles(String),
    #[error("missing runfile for {key:?}: {detail}")]
    MissingRunfile { key: String, detail: String },
    #[error("cannot read manifest {path}: {detail}")]
    Manifest { path: String, detail: String },
    #[error("malformed manifest: {0}")]
    Malformed(String),
    #[error("unsupported manifest schema {found}: want {MANIFEST_SCHEMA_VERSION}")]
    UnsupportedSchema { found: u32 },
    #[error("cannot create scratch {path}: {detail}")]
    Scratch { path: String, detail: String },
    #[error("runner failed: {0}")]
    Runner(String),
    #[error("print_result failed: {0}")]
    Printer(String),
    #[error("invalid quality result: {0}")]
    Result(String),
    #[error("result does not match the manifest: {0}")]
    ResultMismatch(String),
    #[error("printed result does not match the decoded result: {0}")]
    PrintedMismatch(String),
    #[error("cannot render the snapshot difference: {0}")]
    Diff(String),
    #[error("cannot read the pinned snapshot {path}: {detail}")]
    Snapshot { path: String, detail: String },
    #[error("cannot stage the updated snapshot {path}: {detail}")]
    Stage { path: String, detail: String },
    #[error("golden snapshot mismatch at {0}")]
    Mismatch(String),
}
