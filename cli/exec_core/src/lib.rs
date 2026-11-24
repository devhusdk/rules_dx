#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    clippy::wildcard_enum_match_arm
)]

pub mod audit_policy;
pub mod bazel_hint;
pub mod bazel_restart;
pub mod common;
pub mod failure;
pub mod guidance;
pub mod notice_codes;
