#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    clippy::wildcard_enum_match_arm
)]

pub mod build_failures;
pub mod compiler_stderr;
pub mod failure_reports;
pub mod pinned_tool;
pub mod rustc_stderr;
pub mod skipped_targets;
pub mod uv_tools;
