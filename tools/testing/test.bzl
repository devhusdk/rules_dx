"""Rust test macros for the tools/testing package."""

load("@crates//:crates.bzl", "aliases")
load("//rust:defs.bzl", "rust_test")

def testing_rust_test(name, srcs = None, deps = [], **kwargs):
    rust_test(
        name = name,
        srcs = srcs or [name + ".rs"],
        aliases = aliases(
            package_name = "tools/dx_testing",
            normal = True,
            normal_dev = True,
        ),
        crate_name = name,
        crate_root = name + ".rs",
        deps = deps,
        **kwargs
    )
