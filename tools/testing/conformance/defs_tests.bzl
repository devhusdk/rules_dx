"""Tests for the coverage subject settings."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":defs.bzl", "WINDOWS_COVERAGE_OFF", "coverage_subject_settings")

_FEATURES = "//command_line_option:features"

def _checks():
    linux = coverage_subject_settings(["a"], False, "^//x[/:]")
    windows = coverage_subject_settings(["a"], True, "^//x[/:]")
    again = coverage_subject_settings(["-coverage", "a"], True, "^//x[/:]")
    return [
        expect_equal("non-windows keeps features", linux[_FEATURES], ["a"]),
        expect_equal("windows appends coverage off", windows[_FEATURES], ["a"] + WINDOWS_COVERAGE_OFF),
        expect_equal("windows append is idempotent", again[_FEATURES], ["-coverage", "a", "-gcc_coverage_map_format"]),
        expect_equal("filter passes through", windows["//command_line_option:instrumentation_filter"], "^//x[/:]"),
        expect_equal("subject flag set", windows[str(Label("//tools/testing/conformance:subject"))], True),
        expect_equal("coverage on", windows["//command_line_option:collect_code_coverage"], True),
    ]

def conformance_defs_tests(name, size):
    """Declares the conformance defs tests."""
    starlark_test(name = name, size = size, mode = "load", checks = _checks())
