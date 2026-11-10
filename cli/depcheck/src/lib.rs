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
mod dep_key;
mod missing;
mod toml_util;
mod types;
mod unused;
mod usage;
mod version;

pub use consistency::{cmd_consistency, cmd_locks};
pub use ecosystem::beam::{parse_elixir_manifest, parse_erlang_manifest};
pub use ecosystem::cc::{parse_cc_lock, parse_cc_lock_sha, parse_cc_manifest};
pub use ecosystem::dart::{dart_lock_candidates, parse_dart_lock, parse_dart_manifest};
pub use ecosystem::dlang::{parse_d_lock, parse_d_manifest};
pub use ecosystem::dotnet::parse_dotnet_manifest;
pub use ecosystem::fortran::{parse_fortran_lock, parse_fortran_manifest};
pub use ecosystem::go::{parse_go_lock, parse_go_manifest};
pub use ecosystem::haskell::{parse_haskell_lock, parse_haskell_manifest};
pub use ecosystem::js::{parse_js_manifest, parse_pnpm_lock};
pub use ecosystem::julia::{parse_julia_lock, parse_julia_manifest};
pub use ecosystem::jvm::{parse_jvm_lock, parse_jvm_manifest, parse_maven_artifacts_list};
pub use ecosystem::lua::{parse_lua_lock, parse_lua_manifest};
pub use ecosystem::ocaml::{parse_ocaml_lock, parse_ocaml_manifest};
pub use ecosystem::perl::{parse_perl_lock, parse_perl_manifest};
pub use ecosystem::php::{parse_php_lock, parse_php_manifest};
pub use ecosystem::powershell::{parse_powershell_lock, parse_powershell_manifest};
pub use ecosystem::python::{normalize_py, parse_python_lock, parse_python_manifest, satisfies_py};
pub use ecosystem::ruby::parse_ruby_manifest;
pub use ecosystem::rust::{parse_rust_lock, parse_rust_manifest};
pub use ecosystem::swift::{parse_swift_lock, parse_swift_manifest};
pub use ecosystem::zig::{parse_zig_lock, parse_zig_manifest};
pub use ecosystem::{normalize_lower, normalize_snake, parse_paren_lock};
pub use missing::{MissingDep, find_missing, find_missing_go};
pub use types::{DepInfo, DepcheckError, LockVersions, Usage, WorkspaceLocks, language_named};
pub use unused::{DeclaredDep, UnusedDep, declared, find_unused};
pub use usage::{cmd_usage, find_usages, is_test_file};
pub use version::satisfies_for;

#[cfg(test)]
pub(crate) use version::{satisfies_non_semver, versions_equal};

#[cfg(test)]
mod tests;
