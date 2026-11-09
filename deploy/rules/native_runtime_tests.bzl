"""Unit tests for native runtime declarations."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_native_runtime.bzl", "native_runtime_error")

def native_runtime_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "native_runtime_error accepts app_local with files and notice",
                native_runtime_error("14.3", "msvc", "app_local", 1, True),
                "",
            ),
            expect_equal(
                "native_runtime_error accepts os_supplied without files",
                native_runtime_error("2.17", "glibc", "os_supplied", 0, False),
                "",
            ),
            expect_equal(
                "native_runtime_error accepts build_only",
                native_runtime_error("14.3", "msvc", "build_only", 1, False),
                "",
            ),
            expect_equal(
                "native_runtime_error rejects an empty version",
                native_runtime_error("", "msvc", "app_local", 1, True),
                "native_runtime: version must not be empty",
            ),
            expect_equal(
                "native_runtime_error rejects an empty source",
                native_runtime_error("14.3", "", "app_local", 1, True),
                "native_runtime: source must not be empty",
            ),
            expect_equal(
                "native_runtime_error rejects an unknown redistribution",
                native_runtime_error("14.3", "msvc", "bundled", 1, True),
                "native_runtime: unsupported redistribution 'bundled': want one of app_local, os_supplied, build_only",
            ),
            expect_equal(
                "native_runtime_error rejects app_local without files",
                native_runtime_error("14.3", "msvc", "app_local", 0, True),
                "native_runtime: app_local needs at least one file in srcs",
            ),
            expect_equal(
                "native_runtime_error rejects app_local without a notice",
                native_runtime_error("14.3", "msvc", "app_local", 1, False),
                "native_runtime: app_local needs a notice",
            ),
            expect_equal(
                "native_runtime_error rejects os_supplied with files",
                native_runtime_error("2.17", "glibc", "os_supplied", 1, False),
                "native_runtime: os_supplied must not ship files",
            ),
            expect_equal(
                "native_runtime_error accepts os_supplied with provides",
                native_runtime_error("2.17", "glibc", "os_supplied", 0, False, 1),
                "",
            ),
            expect_equal(
                "native_runtime_error rejects provides on app_local",
                native_runtime_error("14.3", "msvc", "app_local", 1, True, 1),
                "native_runtime: only os_supplied may set provides",
            ),
        ],
    )
