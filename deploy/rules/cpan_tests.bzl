"""Unit tests for the CPAN publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_cpan.bzl", "cpan_dist_error", "cpan_name_error", "cpan_version_error")

def cpan_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("cpan_name_error accepts hyphenated names", cpan_name_error("Cpan-Demo"), ""),
            expect_equal(
                "cpan_name_error rejects colons",
                cpan_name_error("Cpan::Demo"),
                "cpan_deploy: invalid package name 'Cpan::Demo': want only [A-Za-z0-9-]",
            ),
            expect_equal(
                "cpan_name_error rejects an empty name",
                cpan_name_error(""),
                "cpan_deploy: invalid package name '': want a non-empty name (for example 'Cpan-Demo')",
            ),
            expect_equal("cpan_version_error accepts dotted versions", cpan_version_error("0.01_1"), ""),
            expect_equal(
                "cpan_version_error rejects version characters",
                cpan_version_error("1.0;curl evil"),
                "cpan_deploy: invalid version '1.0;curl evil': want only [0-9._]",
            ),
            expect_equal("cpan_dist_error accepts a tarball", cpan_dist_error("Cpan-Demo-0.01.tar.gz"), ""),
            expect_equal(
                "cpan_dist_error rejects a zip",
                cpan_dist_error("Cpan-Demo-0.01.zip"),
                "cpan_deploy: invalid dist 'Cpan-Demo-0.01.zip': want a filename ending in .tar.gz",
            ),
        ],
    )
