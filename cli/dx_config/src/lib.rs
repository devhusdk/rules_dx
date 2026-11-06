#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Loads and validates `dx.toml`, and holds the conventions `security.toml` and
//! `licenses.toml` share with it.

pub mod config;
pub mod conventions;
mod curated_defaults_gen;
pub mod schema;
pub mod select;

pub use config::{DxConfig, FamilyConfig, LargeFiles, LoadProblem, Profile, Scope, load};
pub use select::{FamilySelection, Selection, SelectionProblem, effective};

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
#[cfg(test)]
#[path = "conventions_tests.rs"]
mod conventions_tests;
#[cfg(test)]
#[path = "schema_tests.rs"]
mod schema_tests;
#[cfg(test)]
#[path = "select_tests.rs"]
mod select_tests;
