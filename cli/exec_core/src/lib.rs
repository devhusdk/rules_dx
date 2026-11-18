#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    clippy::wildcard_enum_match_arm
)]

//! Shared plumbing for every dx verb: the run environment, event output,
//! failure reporting, next-step guidance and the Bazel restart runner.
//! Verb crates depend on this one and never on each other.

pub mod bazel_hint;
pub mod bazel_restart;
pub mod common;
pub mod failure;
pub mod guidance;
pub mod notice_codes;
