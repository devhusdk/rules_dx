"""Shared toolchain target renderer and hub check tests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//libs/starlark:toolchain.bzl", "hub_checks", "pins_hub", "toolchain_target")

def _checks():
    full = toolchain_target(
        name = "full",
        toolchain = "@full//:toolchain",
        toolchain_type = "//t:type",
        exec_compatible_with = ["//c:a", "//c:b"],
        target_compatible_with = ["//c:c"],
        target_settings = ["//s:on"],
        visibility = ["//visibility:public"],
    )
    minimal = toolchain_target(name = "min", toolchain = "@min//:toolchain", toolchain_type = "//t:type")
    return [
        expect_equal("all attributes", full, """toolchain(
    name = "full",
    exec_compatible_with = ["//c:a", "//c:b"],
    target_compatible_with = ["//c:c"],
    target_settings = ["//s:on"],
    toolchain = "@full//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("omitted attributes", minimal, """toolchain(
    name = "min",
    toolchain = "@min//:toolchain",
    toolchain_type = "//t:type",
)
"""),
        expect_equal("empty list is kept", toolchain_target(name = "e", toolchain = "@e//:t", toolchain_type = "//t:type", exec_compatible_with = []).count("exec_compatible_with = [],"), 1),
        expect_equal("exec hub", pins_hub("t", ["linux_arm64"], "//t:type"), """toolchain(
    name = "t__linux_arm64",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
    toolchain = "@t__linux_arm64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("target hub", pins_hub("t", ["macos_x86_64"], "//t:type", "target"), """toolchain(
    name = "t__macos_x86_64",
    target_compatible_with = ["@platforms//os:macos", "@platforms//cpu:x86_64"],
    toolchain = "@t__macos_x86_64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)
"""),
    ] + hub_checks(full + "\n" + minimal, "//t:type", 2, forbidden = ["@other"])

def toolchain_tests(name):
    """Declares the shared toolchain factory test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
