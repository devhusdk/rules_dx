"""Unit tests for the shared visibility scopes."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":visibility.bzl", "VIS_CLI", "VIS_PUBLIC", "VIS_QUALITY")

def visibility_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "public scope stays one entry",
                VIS_PUBLIC,
                ["//visibility:public"],
            ),
            expect_equal(
                "cli scope stays cli-only",
                VIS_CLI,
                ["//cli:__pkg__", "//cli:__subpackages__"],
            ),
            expect_equal(
                "quality scope stays quality-only",
                VIS_QUALITY,
                ["//quality:__pkg__", "//quality:__subpackages__"],
            ),
        ],
    )
