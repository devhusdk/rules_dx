"""Shared IDE acquisition test macro."""

load("//rust/rules:defs.bzl", "rust_test")

def _ide_rust_test(name, binary, expected, args, compatible_with, size, timeout, kwargs, binary_two = None, expected_two = None, args_two = None):
    """Declares one arch-pinned IDE acquisition test over the shared helper."""
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
        target_compatible_with = rest.pop("target_compatible_with", compatible_with),
        **rest
    )

def ide_acquisition_test(name, expected, version_args = None, binary = None, arm64_binary = None, size = "small", timeout = "short", binary_two = None, expected_two = None, version_args_two = None, **kwargs):
    """Declares one IDE acquisition test over the shared Rust helper."""
    if " " in expected:
        fail("ide_acquisition_test: expected must be one word")
    if binary == None:
        fail("ide_acquisition_test: binary is required")
    if version_args == None:
        version_args = []
    if binary_two != None and arm64_binary != None:
        fail("ide_acquisition_test: binary_two does not support arm64_binary")
    args = " ".join(version_args)
    args_two = " ".join(version_args_two) if version_args_two != None else ""
    if arm64_binary == None:
        _ide_rust_test(name, binary, expected, args, ["@platforms//os:linux"], size, timeout, kwargs, binary_two, expected_two, args_two)
        return
    _ide_rust_test(name + "_aarch64", arm64_binary, expected, args, ["@platforms//os:linux", "@platforms//cpu:arm64"], size, timeout, kwargs)
    _ide_rust_test(name + "_x86_64", binary, expected, args, ["@platforms//os:linux", "@platforms//cpu:x86_64"], size, timeout, kwargs)
    native.test_suite(
        name = name,
        tests = [":" + name + "_aarch64", ":" + name + "_x86_64"],
    )
