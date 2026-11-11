"""Rust foundation pins plus crate manifests."""

load(":lock_registry.bzl", "LOCK_REGISTRY")
load(":versions.bzl", _RUST_VERSION = "RUST_VERSION")

RUST_VERSION = _RUST_VERSION

CRATE_CARGO_LOCK = "//cc/private/context:" + LOCK_REGISTRY["cargo"]["lock"]
CRATE_BAZEL_LOCK = "//:cargo-bazel-lock.json"
