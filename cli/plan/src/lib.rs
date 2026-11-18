//! The planning layer under `dx`: argument parsing, build plans, report
//! planning, target resolution, slots, platform checks, and
//! generated-file projection. `dx_cli`'s exec and adopt layers run on top.

#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

pub mod args;
pub mod disk;
pub mod finalize;
pub mod generate;
pub mod plan;
pub mod platform;
pub mod reports;
pub mod resolve;
pub mod slots;
