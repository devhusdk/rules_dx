#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod bytes;

pub use bytes::{PruneBytes, measure_preserved_bytes, measure_prune_bytes, render_prune_listing};

#[cfg(test)]
mod fixtures;
