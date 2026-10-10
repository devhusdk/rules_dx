#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod gha;
pub mod request;
pub mod sets;
pub mod version;

pub use request::{BumpError, BumpPlanOutcome, BumpRequest};
pub use sets::BumpSet;
pub use version::{compare, generic_major_bump_hint, is_stable, VersionError, WidenVersion};
