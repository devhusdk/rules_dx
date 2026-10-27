// Package rust is the Gazelle extension that generates Rust crate rules.
//
// Test-only modules: a file reached through an outer #[cfg(test)] module
// declaration (or below one) is TestOnly. rustc never reads it for the
// library or binary, so the generated rust_library and rust_binary leave it
// out of srcs, and the crate's unit test becomes self-contained: srcs are
// all files, with crate_root, crate_name, edition and production plus dev
// deps, and no crate=. It carries over compile_data, rustc_env and the other
// crateInheritedAttrs, plus the deps labels of the existing crate rule.
// Crates without TestOnly files keep the crate= unit test. Files reached only
// through an inner #![cfg(test)] stay in srcs. Proc-macro, cdylib, staticlib
// and example targets keep their test modules in srcs and a crate= test.
package rust
