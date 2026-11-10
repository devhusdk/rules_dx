"""Rust foundation pins plus crate manifests."""

load(":versions.bzl", _RUST_VERSION = "RUST_VERSION")

RUST_VERSION = _RUST_VERSION

CRATE_CARGO_LOCK = "//cc/private/context:Cargo.lock"
CRATE_BAZEL_LOCK = "//:cargo-bazel-lock.json"
