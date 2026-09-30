#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod ecosystem;

mod consistency;
mod exceptions;
mod toml_util;
mod types;
mod usage;
mod version;

pub use consistency::{cmd_consistency, cmd_locks};
pub use ecosystem::cc::{normalize_cc, parse_cc_lock, parse_cc_lock_sha, parse_cc_manifest};
pub use ecosystem::dotnet::{normalize_dotnet, parse_dotnet_lock, parse_dotnet_manifest};
pub use ecosystem::go::{normalize_go, parse_go_lock, parse_go_manifest};
pub use ecosystem::js::{normalize_js, parse_js_manifest, parse_pnpm_lock};
pub use ecosystem::jvm::{
    normalize_jvm, parse_jvm_lock, parse_jvm_manifest, parse_maven_artifacts_list,
};
pub use ecosystem::python::{normalize_py, parse_python_lock, parse_python_manifest, satisfies_py};
pub use ecosystem::ruby::{normalize_ruby, parse_ruby_lock, parse_ruby_manifest};
pub use ecosystem::rust::{normalize_rs, parse_rust_lock, parse_rust_manifest};
pub use exceptions::parse_exceptions;
pub use types::{DepInfo, DepcheckError, Ecosystem, Exception, Usage, WorkspaceLocks};
pub use usage::{cmd_usage, find_usages, is_test_file};
pub use version::satisfies;

#[cfg(test)]
pub(crate) use version::versions_equal;

#[cfg(test)]
mod tests;
