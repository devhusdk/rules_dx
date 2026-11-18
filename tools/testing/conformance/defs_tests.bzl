"""Tests for the coverage subject settings and quality configs."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":defs.bzl", "WINDOWS_GCC_COVERAGE_OFF", "coverage_subject_settings", "quality_config")

_FEATURES = "//command_line_option:features"

def _checks():
    linux = coverage_subject_settings(["a"], False, "^//x[/:]")
    windows = coverage_subject_settings(["a"], True, "^//x[/:]")
    again = coverage_subject_settings(["-gcc_coverage_map_format", "a"], True, "^//x[/:]")
    return [
        expect_equal("non-windows keeps features", linux[_FEATURES], ["a"]),
        expect_equal("windows appends gcc coverage off", windows[_FEATURES], ["a"] + WINDOWS_GCC_COVERAGE_OFF),
        expect_equal("windows append is idempotent", again[_FEATURES], ["-gcc_coverage_map_format", "a"]),
        expect_equal("filter passes through", windows["//command_line_option:instrumentation_filter"], "^//x[/:]"),
        expect_equal("subject flag set", windows[str(Label("//tools/testing/conformance:subject"))], True),
        expect_equal("coverage on", windows["//command_line_option:collect_code_coverage"], True),
        expect_equal("string config covers lint", quality_config("haskell", "lint"), "shared"),
        expect_equal("string config covers format", quality_config("haskell", "format"), "shared"),
        expect_equal("per-kind config sets lint", quality_config("scala", "lint"), "shared"),
        expect_equal("per-kind config defaults format", quality_config("scala", "format"), "default"),
        expect_equal("unlisted language uses default", quality_config("go", "lint"), "default"),
    ]

def conformance_defs_tests(name, size):
    """Declares the conformance defs tests."""
    starlark_test(name = name, size = size, mode = "load", checks = _checks())
