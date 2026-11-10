"""Public Rust module extensions."""

load("@rules_rust//crate_universe:extensions.bzl", _crate = "crate")
load("@rules_rust//rust:extensions.bzl", _rust = "rust")

cargo = _crate
rust = _rust
