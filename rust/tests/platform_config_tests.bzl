"""Analysis test that wasm changes the configuration of the Rust binary upstream."""

load("//rust/private:defs.bzl", "RUST_LIBSTDCXX_ATTRS", "rust_binary_transition")
load("//tools/testing:platform_config.bzl", "platform_config_subject_rule", "platform_config_test")

_rust_binary_subject = platform_config_subject_rule(
    rust_binary_transition,
    RUST_LIBSTDCXX_ATTRS | {"wasm": attr.string(doc = "Wasm target, or empty.")},
)

def rust_platform_config_test(name):
    platform_config_test(
        name = name,
        subject_rule = _rust_binary_subject,
        constraints = ["@platforms//os:wasi", "@platforms//cpu:wasm32"],
        subjects = {
            "native": ({}, ""),
            "wasi": ({"wasm": "wasi"}, "wasi,wasm32"),
        },
    )
