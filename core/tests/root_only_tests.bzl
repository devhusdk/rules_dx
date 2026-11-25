"""Tests for the warning root-only module extensions print for non-root tags."""

load("//core:root_only.bzl", "dropped_tag_warnings")
load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")

def _mod(name, is_root, accept = [], toolchain = []):
    return struct(name = name, is_root = is_root, tags = struct(accept = accept, toolchain = toolchain))

def _checks():
    tag = struct(ids = ["cuda-eula"])
    return [
        expect_equal("root tags are read, not warned", dropped_tag_warnings("dx_licenses", [_mod("app", True, accept = [tag])], ["accept"]), []),
        expect_equal("non-root module without tags", dropped_tag_warnings("dx_licenses", [_mod("app", True), _mod("dep", False)], ["accept"]), []),
        expect_equal(
            "nested module tag names the module and tag once",
            dropped_tag_warnings("dx_licenses", [_mod("app", True), _mod("dep", False, accept = [tag, tag])], ["accept"]),
            ["dx_licenses: module 'dep' sets dx_licenses.accept(...), which is ignored because only the root module's accept tags count; move the tag to the root MODULE.bazel to apply it"],
        ),
        expect_equal(
            "one warning per module and tag class",
            dropped_tag_warnings("python", [_mod("app", True), _mod("a", False, toolchain = [tag]), _mod("b", False, accept = [tag], toolchain = [tag])], ["toolchain"]),
            [
                "python: module 'a' sets python.toolchain(...), which is ignored because only the root module's toolchain tags count; move the tag to the root MODULE.bazel to apply it",
                "python: module 'b' sets python.toolchain(...), which is ignored because only the root module's toolchain tags count; move the tag to the root MODULE.bazel to apply it",
            ],
        ),
    ]

def root_only_tests(name, size):
    """Declares the root-only extension warning test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
