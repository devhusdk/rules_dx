"""Unit tests for the shell-harness helper."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":harness.bzl", "dx_harness_data", "dx_harness_linux", "dx_harness_tags")

def harness_unit_tests(name):
    """Runs the unit tests for the shell-harness helper."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "bare data resolves to the shared bootstrap pair",
                dx_harness_data(),
                ["//tools/sh:bootstrap", "//tools/sh:lib"],
            ),
            expect_equal(
                "caller data keeps its entries ahead of the shared pair",
                dx_harness_data([":version"]),
                [":version", "//tools/sh:bootstrap", "//tools/sh:lib"],
            ),
            expect_equal(
                "bare tags resolve to the no-coverage marker",
                dx_harness_tags(),
                ["no-coverage"],
            ),
            expect_equal(
                "caller tags keep their entries ahead of the marker",
                dx_harness_tags(["requires-network"]),
                ["requires-network", "no-coverage"],
            ),
            expect_equal(
                "existing marker is not duplicated",
                dx_harness_tags(["no-coverage"]),
                ["no-coverage"],
            ),
            expect_equal(
                "bare constraint resolves to Linux-only",
                dx_harness_linux(),
                ["@platforms//os:linux"],
            ),
            expect_equal(
                "explicit constraint passes through untouched",
                dx_harness_linux(["@platforms//os:macos"]),
                ["@platforms//os:macos"],
            ),
        ],
    )
