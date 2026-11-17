"""Shared toolchain target renderer and hub check tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:toolchain.bzl", "hub_checks", "pins_hub", "toolchain_target")

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
        expect_equal("target hub with an exec toolchain type", pins_hub("t", ["linux_x86_64"], "//t:type", "target", exec_toolchain_type = "//t:exec_type"), """toolchain(
    name = "t__linux_x86_64",
    target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    toolchain = "@t__linux_x86_64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)

toolchain(
    name = "t__linux_x86_64_exec",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    toolchain = "@t__linux_x86_64//:toolchain",
    toolchain_type = "//t:exec_type",
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("os constraints follow the cell platform", pins_hub("t", ["windows_x86_64", "linux_x86_64"], "//t:type", "both", os_constraints = {"windows": ["//abi:msvc"]}), """toolchain(
    name = "t__linux_x86_64",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    toolchain = "@t__linux_x86_64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)

toolchain(
    name = "t__windows_x86_64",
    exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64", "//abi:msvc"],
    target_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64", "//abi:msvc"],
    toolchain = "@t__windows_x86_64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("both hub with exec aliases", pins_hub("t", ["linux_x86_64"], "//t:type", "both", ["bin"]), """toolchain(
    name = "t__linux_x86_64",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    toolchain = "@t__linux_x86_64//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)

config_setting(
    name = "linux_x86_64_exec",
    constraint_values = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
)

alias(
    name = "bin",
    actual = select({":linux_x86_64_exec": "@t__linux_x86_64//:bin"}),
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("variants with a companion type and an alias default", pins_hub("t", ["macos_arm64"], "//t:type", "both", {"lib": "//:a/lib"}, variants = {"v": struct(exec_compatible_with = ["//e:v"], target_compatible_with = ["//x:v"])}, companion_types = {"profile": "//t:profile"}, alias_defaults = {"lib": ":none"}), """toolchain(
    name = "t__macos_arm64_v",
    exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64", "//e:v"],
    target_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64", "//x:v"],
    toolchain = "@t__macos_arm64//:v",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)

toolchain(
    name = "t__macos_arm64_v_profile",
    exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64", "//e:v"],
    target_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64", "//x:v"],
    toolchain = "@t__macos_arm64//:v_profile",
    toolchain_type = "//t:profile",
    visibility = ["//visibility:public"],
)

config_setting(
    name = "macos_arm64_exec",
    constraint_values = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
)

alias(
    name = "lib",
    actual = select({":macos_arm64_exec": "@t__macos_arm64//:a/lib", "//conditions:default": ":none"}),
    visibility = ["//visibility:public"],
)
"""),
    ] + hub_checks(full + "\n" + minimal, "//t:type", 2, forbidden = ["@other"])

def toolchain_tests(name, size = "small"):
    """Declares the shared toolchain factory test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
