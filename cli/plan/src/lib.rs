//! The planning layer under `dx`: argument parsing, build plans, report
//! planning, target resolution, slots, platform checks, and
//! generated-file projection. dx_cli's exec and adopt layers run on top.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod args;
pub mod disk;
pub mod finalize;
pub mod generate;
pub mod plan;
pub mod platform;
pub mod reports;
pub mod resolve;
pub mod slots;
