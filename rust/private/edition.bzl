"""Rust edition for every crate target, from //modules:versions.bzl."""

load("//modules:versions.bzl", _RUST_EDITION = "RUST_EDITION")

RUST_EDITION = _RUST_EDITION
