"""Wrapper conformance tests for Zig."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "ZIG_QUALITY_TOOLS", "zig_import_name", "zig_quality_srcs")

def zig_wrapper_contract_tests(name):
    """Instantiates Zig wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["zig"],
        extra_checks = [
            expect_equal(
                "wrappers carry zig_fmt",
                sorted(ZIG_QUALITY_TOOLS.keys()),
                ["zig_fmt"],
            ),
            expect_equal(
                "main leads the owned sources",
                zig_quality_srcs("main.zig", ["util.zig"]),
                ["main.zig", "util.zig"],
            ),
            expect_equal(
                "main listed in srcs is owned once",
                zig_quality_srcs("main.zig", ["main.zig", "util.zig"]),
                ["main.zig", "util.zig"],
            ),
            expect_equal(
                "root-module binaries own no direct sources",
                zig_quality_srcs(None, None),
                [],
            ),
            expect_equal(
                "import name defaults to the public target name",
                zig_import_name("greet", None),
                "greet",
            ),
            expect_equal(
                "explicit import name wins",
                zig_import_name("greet_lib", "greet"),
                "greet",
            ),
        ],
    )
