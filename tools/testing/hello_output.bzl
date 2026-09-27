"""Shared hello-output assertion over rust_test."""

load("//rust/rules:defs.bzl", "rust_test")

def dx_hello_output_test(name, binary, expected, exit_code = 0, size = "small", timeout = "short", **kwargs):
    """Declares one fixture-binary output assertion over the shared helper."""
    rust_test(
        name = name,
        srcs = ["//tools/testing:hello_output_test.rs"],
        data = [binary],
        deps = ["//tools/testing:dx_testing"],
        env = {
            "DX_HELLO_BIN": "$(rootpath %s)" % binary,
            "DX_HELLO_EXPECTED": expected,
            "DX_HELLO_EXPECTED_CODE": str(exit_code),
        },
        size = size,
        timeout = timeout,
        **kwargs
    )
