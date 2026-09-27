"""Shared IDE acquisition test macro."""

load("//tools/sh:harness.bzl", "dx_shell_harness")

def ide_acquisition_test(name, expected, version_args = None, binary = None, arm64_binary = None, **kwargs):
    """Declares one IDE acquisition test over the shared script."""
    if " " in expected:
        fail("ide_acquisition_test: expected must be one word")
    if binary == None:
        fail("ide_acquisition_test: binary is required")
    if version_args == None:
        version_args = []
    tail = [expected] + version_args
    if arm64_binary != None:
        args = select({
            "@platforms//cpu:arm64": ["$(rootpath " + arm64_binary + ")"] + tail,
            "//conditions:default": ["$(rootpath " + binary + ")"] + tail,
        })
        data = select({
            "@platforms//cpu:arm64": [arm64_binary],
            "//conditions:default": [binary],
        })
    else:
        args = ["$(rootpath " + binary + ")"] + tail
        data = [binary]
    dx_shell_harness(
        name = name,
        srcs = ["//tools/ide:ide_acquisition.sh"],
        args = args,
        data = data,
        **kwargs
    )
