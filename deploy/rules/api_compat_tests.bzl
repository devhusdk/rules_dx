"""Unit tests for the shared api_compat attributes."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")

def api_compat_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("api_compat_error accepts the defaults", api_compat_error("x_deploy", "fail", "registry"), ""),
            expect_equal("api_compat_error accepts warn", api_compat_error("x_deploy", "warn", "git"), ""),
            expect_equal(
                "api_compat_error rejects a bool",
                api_compat_error("x_deploy", True, "git"),
                "x_deploy: invalid api_compat 'True': want 'off', 'warn' or 'fail'",
            ),
            expect_equal(
                "api_compat_error rejects an unknown mode",
                api_compat_error("x_deploy", "loud", "registry"),
                "x_deploy: invalid api_compat 'loud': want 'off', 'warn' or 'fail'",
            ),
            expect_equal(
                "api_compat_error rejects an unknown baseline",
                api_compat_error("x_deploy", "fail", "tag"),
                "x_deploy: invalid api_baseline 'tag': want 'registry' or 'git'",
            ),
            expect_equal(
                "the defaults are fail and registry",
                [API_COMPAT_ATTRS["api_compat"] != None, API_COMPAT_ATTRS["api_baseline"] != None],
                [True, True],
            ),
            expect_equal(
                "every library rule shares api_compat and api_baseline",
                sorted(API_COMPAT_ATTRS.keys()),
                ["api_baseline", "api_compat"],
            ),
        ],
    )
