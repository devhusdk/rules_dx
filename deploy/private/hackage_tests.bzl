"""Unit tests for the Hackage publisher."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate_hackage.bzl", "hackage_name_error", "hackage_sdist_error", "hackage_version_error")

def hackage_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "hackage_name_error accepts hyphenated names",
                hackage_name_error("hackage-demo"),
                "",
            ),
            expect_equal(
                "hackage_name_error rejects an empty name",
                hackage_name_error(""),
                "hackage_deploy: invalid package name '': want a non-empty " +
                "name (for example 'hackage_demo')",
            ),
            expect_equal(
                "hackage_name_error rejects underscores",
                hackage_name_error("hackage_demo"),
                "hackage_deploy: invalid package name 'hackage_demo': want " +
                "only [A-Za-z0-9-]",
            ),
            expect_equal(
                "hackage_version_error accepts dotted versions",
                hackage_version_error("0.1.0"),
                "",
            ),
            expect_equal(
                "hackage_version_error rejects an empty version",
                hackage_version_error(""),
                "hackage_deploy: invalid version '': want a non-empty " +
                "version (for example '0.1.0')",
            ),
            expect_equal(
                "hackage_version_error rejects version characters",
                hackage_version_error("1.0;curl evil"),
                "hackage_deploy: invalid version '1.0;curl evil': want only [0-9.]",
            ),
            expect_equal(
                "hackage_sdist_error accepts a tarball",
                hackage_sdist_error("hackage-demo-0.1.0.tar.gz"),
                "",
            ),
            expect_equal(
                "hackage_sdist_error rejects a zip",
                hackage_sdist_error("hackage-demo-0.1.0.zip"),
                "hackage_deploy: invalid sdist 'hackage-demo-0.1.0.zip': " +
                "want a filename ending in .tar.gz",
            ),
        ],
    )
