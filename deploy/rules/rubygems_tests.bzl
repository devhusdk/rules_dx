"""Unit and analysis tests for the rubygems.io publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_rubygems.bzl", "rubygems_name_error", "rubygems_version_error")

def rubygems_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "rubygems_name_error accepts the demo gem name",
                rubygems_name_error("rubygems_demo"),
                "",
            ),
            expect_equal(
                "rubygems_name_error accepts dashed and dotted names",
                rubygems_name_error("dx-plugin.core"),
                "",
            ),
            expect_equal(
                "rubygems_name_error rejects an empty name",
                rubygems_name_error(""),
                "rubygems_deploy: invalid gem name '': want a non-empty " +
                "name (for example 'rubygems_demo')",
            ),
            expect_equal(
                "rubygems_name_error rejects punctuation in gem names",
                rubygems_name_error("dx!plugin"),
                "rubygems_deploy: invalid gem name 'dx!plugin': want " +
                "only [A-Za-z0-9_.-]",
            ),
            expect_equal(
                "rubygems_name_error rejects name characters",
                rubygems_name_error("dx;curl evil"),
                "rubygems_deploy: invalid gem name 'dx;curl evil': want " +
                "only [A-Za-z0-9_.-]",
            ),
            expect_equal(
                "rubygems_version_error accepts the demo version",
                rubygems_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "rubygems_version_error accepts prerelease versions",
                rubygems_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "rubygems_version_error rejects an empty version",
                rubygems_version_error(""),
                "rubygems_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "rubygems_version_error rejects version characters",
                rubygems_version_error("1.0;curl evil"),
                "rubygems_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
        ],
    )
