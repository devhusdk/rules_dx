#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The planning layer under `dx`: argument parsing, build plans, report
//! planning, target resolution, version skew, slots, platform checks, and
//! generated-file projection. dx_cli's exec and adopt layers run on top.

pub mod args;
pub mod disk;
pub mod finalize;
pub mod generate;
pub mod plan;
pub mod platform;
pub mod reports;
pub mod resolve;
pub mod skew;
pub mod slots;
