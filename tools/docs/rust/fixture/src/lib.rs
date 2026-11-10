//! Shapes and lengths for the docs site API reference.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod shapes;

pub use shapes::Circle;

/// Adds two lengths in meters.
pub fn add(left: u64, right: u64) -> u64 {
    left + private_offset(right)
}

fn private_offset(value: u64) -> u64 {
    value
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
