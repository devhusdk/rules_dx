"""Shared toolchain target renderer and hub check tests."""

load("//core/private/testing:defs.bzl", "expect_equal", "expect_false", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "digest_problems", "hub_checks", "is_sha256", "pins_hub", "toolchain_target")

_DIGEST = "0123456789abcdef" * 4

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
        expect_true("a lowercase digest is sha256", is_sha256(_DIGEST)),
        expect_false("an uppercase digest is not sha256", is_sha256(_DIGEST.upper())),
        expect_false("a 63 character digest is not sha256", is_sha256(_DIGEST[1:])),
        expect_false("a non-string digest is not sha256", is_sha256(None)),
        expect_equal("a nested bad entry is named by its path", digest_problems("p", {"cells": {"a": {"sha256": _DIGEST.upper(), "urls": ["http://x"]}}}), [
            "p.cells.a: sha256 is not 64 lowercase hex characters",
            "p.cells.a: urls is not a non-empty list of https urls",
        ]),
        expect_equal("empty urls are a problem", digest_problems("p", {"sha256": _DIGEST, "urls": []}), ["p: urls is not a non-empty list of https urls"]),
        expect_equal("an entry without urls passes", digest_problems("p", {"hex": {"a": {"sha256": _DIGEST}}}), []),
        expect_equal("a dict of digests passes", digest_problems("p", {"sha256": {"x86_64": _DIGEST, "arm64": _DIGEST}, "urls": ["https://x"]}), []),
        expect_equal("lists are walked", digest_problems("p", {"tools": [{"sha256": _DIGEST}, {"sha256": "bad"}]}), ["p.tools[1]: sha256 is not 64 lowercase hex characters"]),
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
        expect_equal("cells sharing one repository", pins_hub("t", ["linux_x86_64", "macos_arm64"], "//t:type", repository = "shared"), """toolchain(
    name = "t__linux_x86_64",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    toolchain = "@shared//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)

toolchain(
    name = "t__macos_arm64",
    exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
    toolchain = "@shared//:toolchain",
    toolchain_type = "//t:type",
    visibility = ["//visibility:public"],
)
"""),
        expect_equal("gated cell with its own constraints", pins_hub(
            "t",
            ["linux_arm64_to_linux_x86_64"],
            "//t:type",
            "both",
            target = "impl",
            target_settings = ["//t:on"],
            cell_constraints = {"linux_arm64_to_linux_x86_64": struct(exec_compatible_with = ["//c:arm64"], target_compatible_with = ["//c:x86_64"])},
        ), """toolchain(
    name = "t__linux_arm64_to_linux_x86_64",
    exec_compatible_with = ["//c:arm64"],
    target_compatible_with = ["//c:x86_64"],
    target_settings = ["//t:on"],
    toolchain = "@t__linux_arm64_to_linux_x86_64//:impl",
    toolchain_type = "//t:type",
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
