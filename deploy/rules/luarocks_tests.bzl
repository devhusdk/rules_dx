"""Unit tests for the LuaRocks publisher."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate_luarocks.bzl", "luarocks_name_error", "luarocks_rockspec_error", "luarocks_version_error")

def luarocks_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "luarocks_name_error accepts the demo name",
                luarocks_name_error("luarocks_demo"),
                "",
            ),
            expect_equal(
                "luarocks_name_error rejects an empty name",
                luarocks_name_error(""),
                "luarocks_deploy: invalid package name '': want a non-empty " +
                "name (for example 'luarocks_demo')",
            ),
            expect_equal(
                "luarocks_name_error rejects name characters",
                luarocks_name_error("dx;curl evil"),
                "luarocks_deploy: invalid package name 'dx;curl evil': want " +
                "only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "luarocks_version_error accepts a revision",
                luarocks_version_error("0.1.0-1"),
                "",
            ),
            expect_equal(
                "luarocks_version_error rejects an empty version",
                luarocks_version_error(""),
                "luarocks_deploy: invalid version '': want a non-empty " +
                "version (for example '0.1.0' or '0.1.0-1')",
            ),
            expect_equal(
                "luarocks_rockspec_error accepts a rockspec",
                luarocks_rockspec_error("luarocks_demo-0.1.0-1.rockspec"),
                "",
            ),
            expect_equal(
                "luarocks_rockspec_error rejects other files",
                luarocks_rockspec_error("luarocks_demo.lua"),
                "luarocks_deploy: invalid rockspec 'luarocks_demo.lua': " +
                "want a filename ending in .rockspec",
            ),
        ],
    )
