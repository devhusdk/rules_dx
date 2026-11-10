"""Rust edition for every crate target, from //core/modules:versions.bzl."""

load("//core/modules:versions.bzl", _RUST_EDITION = "RUST_EDITION")

RUST_EDITION = _RUST_EDITION
