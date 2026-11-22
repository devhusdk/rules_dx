#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    clippy::wildcard_enum_match_arm
)]

//! Report helpers shared by the dx verbs: build failure records and the
//! failures report, compiler diagnostics from failed actions, the
//! skipped-target notice and pinned tool resolution.

pub mod build_failures;
pub mod compiler_stderr;
pub mod failure_reports;
pub mod pinned_tool;
pub mod rustc_stderr;
pub mod skipped_targets;
pub mod uv_tools;
