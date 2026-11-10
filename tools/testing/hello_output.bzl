"""Shared hello-output assertion over rust_test."""

load("//rust/rules:defs.bzl", "rust_test")

def dx_hello_output_test(name, binary, expected, exit_code = 0, size = "small", timeout = "short", condition_env = {}, condition_data = {}, **kwargs):
    """Declares one fixture-binary output assertion over the shared helper."""
    env = {
        "DX_HELLO_BIN": "$(rootpath %s)" % binary,
        "DX_HELLO_EXPECTED": expected,
        "DX_HELLO_EXPECTED_CODE": str(exit_code),
    }
    data = [binary]
    if condition_env:
        env = select(dict({c: dict(env, **extra) for c, extra in condition_env.items()}, **{"//conditions:default": env}))
    if condition_data:
        data = data + select(dict({c: extra for c, extra in condition_data.items()}, **{"//conditions:default": []}))
    rust_test(
        name = name,
        srcs = [Label("//tools/testing:hello_output_test.rs")],
        data = data,
        deps = [Label("//tools/testing:dx_testing")],
        env = env,
        size = size,
        timeout = timeout,
        **kwargs
    )
