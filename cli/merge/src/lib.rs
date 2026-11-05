#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Pure text merges that change only what dx owns and keep every other byte.

pub mod block;
pub mod editorconfig;
pub mod jsonc;
pub mod yaml_scalar;

/// Why a merge could not run.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The text has a begin marker without an end marker.
    #[error("found {begin} without {end}")]
    OrphanedMarker { begin: String, end: String },
    /// The text is not valid JSONC.
    #[error("not valid JSONC: {0}")]
    Jsonc(String),
    /// The JSONC value on the path is not an object or array as needed.
    #[error("`{0}` has the wrong type")]
    WrongType(String),
    /// The key is not in the YAML text.
    #[error("no `{0}:` line found")]
    MissingKey(String),
}
