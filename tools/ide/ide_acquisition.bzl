"""Shared IDE acquisition test macro."""

load("//rust/rules:defs.bzl", "rust_test")

def _ide_rust_test(name, binary, expected, args, size, timeout, kwargs, binary_two = None, expected_two = None, args_two = None):
    """Declares one IDE acquisition test over the shared helper."""
    env = {
        "DX_IDE_ARGS": args,
        "DX_IDE_BIN": "$(rootpath " + binary + ")",
        "DX_IDE_EXPECTED": expected,
    }
    data = [binary]
    if binary_two != None:
        if expected_two == None:
            fail("ide_acquisition_test: expected_two is required with binary_two")
        env["DX_IDE_ARGS_TWO"] = args_two or ""
        env["DX_IDE_BIN_TWO"] = "$(rootpath " + binary_two + ")"
        env["DX_IDE_EXPECTED_TWO"] = expected_two
        data.append(binary_two)
    rest = dict(kwargs)
    rust_test(
        name = name,
        srcs = ["//tools/ide:ide_acquisition_test.rs"],
        data = data,
        deps = ["//tools/testing:dx_testing"],
        env = env,
        size = rest.pop("size", size),
        timeout = rest.pop("timeout", timeout),
        **rest
    )

def ide_acquisition_test(name, expected, version_args = None, binary = None, size = "small", timeout = "short", binary_two = None, expected_two = None, version_args_two = None, **kwargs):
    """Declares one IDE acquisition test over the shared Rust helper.

    Callers that need an OS or CPU limit pass their own target_compatible_with.
    """
    if " " in expected:
        fail("ide_acquisition_test: expected must be one word")
    if binary == None:
        fail("ide_acquisition_test: binary is required")
    if version_args == None:
        version_args = []
    args = " ".join(version_args)
    args_two = " ".join(version_args_two) if version_args_two != None else ""
    _ide_rust_test(name, binary, expected, args, size, timeout, kwargs, binary_two, expected_two, args_two)
