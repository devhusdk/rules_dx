// Package rust is the Gazelle extension that generates Rust crate rules.
//
// Test-only modules: a file reached through an outer #[cfg(test)] module
// declaration (or below one) is TestOnly. rustc never reads it for the
// library or binary, so the generated rust_library and rust_binary leave it
// out of srcs, and the crate's unit test becomes self-contained: srcs are
// all files, with crate_root, crate_name, edition and production plus dev
// deps, and no crate=. It carries over compile_data, rustc_env and the other
// crateInheritedAttrs, plus the deps labels of the existing crate rule, and
// sets coverage_lib to its library so coverage counts the library's sources.
// An existing test value that differs from the crate's is an error, except a
// compile_data list that holds every crate entry: the test keeps it.
// Under map_kind Gazelle never adds coverage_lib but keeps an existing one; a
// hand-written coverage_lib on a test Gazelle does not set it on (binary or
// integration tests) needs # keep.
// Crates without TestOnly files keep the crate= unit test. Files reached only
// through an inner #![cfg(test)] stay in srcs. Proc-macro, cdylib, staticlib
// and example targets keep their test modules in srcs and a crate= test.
//
// Target-gated dependencies: external crates under [target.*.dependencies] are
// gated by crate_universe through all_crate_deps. Local path deps are gated by
// a select on os or rules_rust triple config_settings; a cfg that matches no
// supported target is an error.
//
// Crate hubs: aliases and all_crate_deps come from @crates//:crates.bzl unless
// # gazelle:dx_rust_crates_hub <repo> names another crate_universe hub for the
// package and its subpackages. Generation then writes the
// @<repo>//:crates.bzl load into the existing BUILD file, with all_crate_deps
// only while a deps call names external crates. A package with the directive
// but no BUILD file is an error. Moving a package from one non-default hub to
// another leaves the old hub's load in place; delete it by hand.
package rust
