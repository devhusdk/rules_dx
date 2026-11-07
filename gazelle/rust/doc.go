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
// Crates without TestOnly files keep the crate= unit test. Files reached only
// through an inner #![cfg(test)] stay in srcs. Proc-macro, cdylib, staticlib
// and example targets keep their test modules in srcs and a crate= test.
//
// Target-gated dependencies: external crates under [target.*.dependencies] are
// gated by crate_universe through all_crate_deps. Local path deps are gated by
// a select on os or rules_rust triple config_settings; a cfg that matches no
// supported target is an error.
package rust
