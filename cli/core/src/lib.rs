#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
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
pub mod skew;
pub mod slots;
