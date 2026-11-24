#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

pub mod block;
pub mod jsonc;
pub mod merge3;
pub mod yaml_scalar;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("found {begin} without {end}")]
    OrphanedMarker { begin: String, end: String },
    #[error("not valid JSONC: {0}")]
    Jsonc(String),
    #[error("`{0}` has the wrong type")]
    WrongType(String),
}
