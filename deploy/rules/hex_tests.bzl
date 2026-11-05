"""Unit tests for the Hex publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":hex.bzl", "hex_name_error", "hex_tool_error", "hex_version_error")

def hex_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "hex_name_error accepts the demo package name",
                hex_name_error("hex_demo"),
                "",
            ),
            expect_equal(
                "hex_name_error rejects an empty name",
                hex_name_error(""),
                "hex_deploy: invalid package name '': want a non-empty " +
                "name (for example 'hex_demo')",
            ),
            expect_equal(
                "hex_name_error rejects dashes",
                hex_name_error("hex-demo"),
                "hex_deploy: invalid package name 'hex-demo': want only [A-Za-z0-9_]",
            ),
            expect_equal(
                "hex_version_error accepts prerelease versions",
                hex_version_error("1.2.3-rc.1"),
                "",
            ),
            expect_equal(
                "hex_version_error rejects an empty version",
                hex_version_error(""),
                "hex_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "hex_version_error rejects version characters",
                hex_version_error("1.0;curl evil"),
                "hex_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
            expect_equal(
                "hex_tool_error accepts mix and rebar3",
                [hex_tool_error("mix"), hex_tool_error("rebar3")],
                ["", ""],
            ),
            expect_equal(
                "hex_tool_error rejects other tools",
                hex_tool_error("gleam"),
                "hex_deploy: invalid tool 'gleam': want one of mix, rebar3",
            ),
        ],
    )
