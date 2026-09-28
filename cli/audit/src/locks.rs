#[path = "locks_cargo.rs"]
mod locks_cargo;
#[path = "locks_go.rs"]
mod locks_go;
#[path = "locks_licenses.rs"]
mod locks_licenses;
#[path = "locks_maven.rs"]
mod locks_maven;
#[path = "locks_npm.rs"]
mod locks_npm;
#[path = "locks_paket.rs"]
mod locks_paket;
#[path = "locks_pnpm.rs"]
mod locks_pnpm;
#[path = "locks_yarn.rs"]
mod locks_yarn;

pub use locks_cargo::*;
pub use locks_go::*;
pub use locks_licenses::*;
pub use locks_maven::*;
pub use locks_npm::*;
pub use locks_paket::*;
pub use locks_pnpm::*;
pub use locks_yarn::*;

#[cfg(test)]
pub(crate) use locks_npm::package_lock_name;
#[cfg(test)]
pub(crate) use locks_paket::{split_paket_line, split_paket_line_fallback};
#[cfg(test)]
pub(crate) use locks_pnpm::{split_pnpm_scoped_fallback, split_pnpm_unscoped_fallback};

#[cfg(test)]
#[path = "locks_tests.rs"]
mod locks_tests;
