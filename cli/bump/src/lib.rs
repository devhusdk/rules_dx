#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod discovery;
pub mod gha;
pub mod request;
pub mod sets;
pub mod version;

pub use request::{BumpError, BumpRequest};
pub use sets::BumpSet;
pub use version::{
    compare, generic_major_bump_hint, is_major_bump, is_stable, major_bump_migrate_hint,
    VersionError, WidenVersion,
};
