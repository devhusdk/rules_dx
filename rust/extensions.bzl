"""Public Rust module extensions."""

load("@rules_rust//rust:extensions.bzl", _rust = "rust")
load("//rust/private/toolchain:hub.bzl", _dx_rust_toolchains = "dx_rust_toolchains")
load("//third_party/rules_rust/crate_universe:extensions.bzl", _crate = "crate")

cargo = _crate
rust = _rust
rust_toolchains = _dx_rust_toolchains
