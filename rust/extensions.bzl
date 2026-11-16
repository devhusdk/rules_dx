"""Public Rust module extensions."""

load("@rules_rust//rust:extensions.bzl", _rust = "rust")
load("//third_party/rules_rust/crate_universe:extensions.bzl", _crate = "crate")

cargo = _crate
rust = _rust
