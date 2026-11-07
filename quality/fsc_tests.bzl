"""Unit tests for the warnings the fsc typecheck turns on."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":fsc.bzl", "fsc_warnon_args")

def fsc_unit_tests(name):
    """Declares the fsc unit tests."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "unused values are reported by default",
                fsc_warnon_args([], []),
                ["--warnon:1182"],
            ),
            expect_equal(
                "nowarn 1182 keeps unused values off",
                fsc_warnon_args(["1182"], []),
                [],
            ),
            expect_equal(
                "nowarn FS1182 keeps unused values off",
                fsc_warnon_args(["FS1182"], []),
                [],
            ),
            expect_equal(
                "a --nowarn:1182 compiler option keeps unused values off",
                fsc_warnon_args([], ["--nowarn:1182"]),
                [],
            ),
        ],
    )
