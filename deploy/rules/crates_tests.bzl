"""Unit and analysis tests for the crates.io publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":crates.bzl", "crates_name_error", "crates_version_error")

def crates_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "crates_name_error accepts the demo crate name",
                crates_name_error("crates_demo"),
                "",
            ),
            expect_equal(
                "crates_name_error accepts dashed names",
                crates_name_error("dx-plugin"),
                "",
            ),
            expect_equal(
                "crates_name_error rejects an empty name",
                crates_name_error(""),
                "crates_deploy: invalid crate name '': want a non-empty " +
                "name (for example 'crates_demo')",
            ),
            expect_equal(
                "crates_name_error rejects dots in crate names",
                crates_name_error("dx.plugin"),
                "crates_deploy: invalid crate name 'dx.plugin': want " +
                "only [A-Za-z0-9_-]",
            ),
            expect_equal(
                "crates_name_error rejects name characters",
                crates_name_error("dx;curl evil"),
                "crates_deploy: invalid crate name 'dx;curl evil': want " +
                "only [A-Za-z0-9_-]",
            ),
            expect_equal(
                "crates_version_error accepts the demo version",
                crates_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "crates_version_error accepts prerelease versions",
                crates_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "crates_version_error rejects an empty version",
                crates_version_error(""),
                "crates_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "crates_version_error rejects version characters",
                crates_version_error("1.0;curl evil"),
                "crates_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
        ],
    )
